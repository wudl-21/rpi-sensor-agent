(() => {
  const $ = (id) => document.getElementById(id);
  const state = { client: null, devices: new Map(), messages: 0, commands: 0, paused: false };
  const topics = ['devices/+/status', 'devices/+/telemetry', 'devices/+/events', 'devices/+/command_ack'];

  const saved = {
    host: localStorage.getItem('sensor-broker-host') || `ws://${location.hostname || 'localhost'}:9001/mqtt`,
    user: localStorage.getItem('sensor-broker-user') || 'sensor-console',
    password: localStorage.getItem('sensor-broker-password') || ''
  };
  $('brokerHost').value = saved.host;
  $('mqttUser').value = saved.user;
  $('mqttPassword').value = saved.password;

  function toast(message) {
    const node = $('toast');
    node.textContent = message;
    node.classList.add('show');
    setTimeout(() => node.classList.remove('show'), 3200);
  }

  function setConnection(online, text) {
    $('connectionDot').className = `status-dot ${online ? 'online' : 'offline'}`;
    $('connectionText').textContent = text;
  }

  function timeLabel(timestamp) {
    return new Date(timestamp || Date.now()).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function deviceIdFromTopic(topic) {
    const parts = topic.split('/');
    return parts[1] || 'unknown';
  }

  function addActivity(topic, payload, status = '') {
    if (state.paused) return;
    const list = $('activity');
    list.querySelector('.activity-empty')?.remove();
    const row = document.createElement('div');
    row.className = 'activity-item';
    row.innerHTML = `<span class="activity-time">${timeLabel()}</span><span class="activity-topic">${topic}</span><span class="activity-payload">${escapeHtml(payload)}</span><span class="activity-status">${status}</span>`;
    list.prepend(row);
    while (list.children.length > 80) list.lastElementChild.remove();
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#039;' }[char]));
  }

  function parsePayload(message) {
    try { return JSON.parse(message); } catch { return null; }
  }

  function touchDevice(id, patch) {
    const current = state.devices.get(id) || { id, online: false, version: '--', lastSeen: 0, temperature: null };
    state.devices.set(id, { ...current, ...patch, lastSeen: Date.now() });
    renderDevices();
  }

  function renderDevices() {
    const devices = [...state.devices.values()].sort((a, b) => a.id.localeCompare(b.id));
    $('deviceCount').textContent = devices.length;
    $('onlineCount').textContent = devices.filter((device) => device.online).length;
    $('deviceRows').innerHTML = devices.length ? devices.map((device) => `
      <tr><td><span class="device-name">${escapeHtml(device.id)}</span><span class="device-sub">${device.target || 'sensor endpoint'}</span></td>
      <td><span class="state ${device.online ? 'online' : 'offline'}">● ${device.online ? 'ONLINE' : 'OFFLINE'}</span></td>
      <td>${escapeHtml(device.version || '--')}<span class="device-sub">seq ${device.releaseSeq ?? '--'}</span></td>
      <td>${device.lastSeen ? timeLabel(device.lastSeen) : '--'}</td><td><button class="select-device" data-device="${escapeHtml(device.id)}">USE</button></td></tr>`).join('') : '<tr class="empty-row"><td colspan="5">Waiting for devices to speak...</td></tr>';
    $('deviceOptions').innerHTML = devices.map((device) => `<option value="${escapeHtml(device.id)}">`).join('');
    document.querySelectorAll('.select-device').forEach((button) => button.addEventListener('click', () => { $('deviceId').value = button.dataset.device; }));
  }

  function handleMessage(topic, raw) {
    state.messages += 1;
    $('messageCount').textContent = state.messages;
    const payload = parsePayload(raw);
    const id = deviceIdFromTopic(topic);
    const kind = topic.split('/')[2];
    addActivity(topic, raw, payload?.status || '');
    if (!payload) return;
    if (kind === 'status') {
      touchDevice(id, { online: payload.event === 'online', version: payload.version, releaseSeq: payload.release_seq, target: payload.target });
    } else if (kind === 'telemetry') {
      touchDevice(id, { online: true });
      $('temperature').textContent = payload.temperature == null ? '--' : `${Number(payload.temperature).toFixed(1)}°`;
      $('temperatureDevice').textContent = `${id} · ${timeLabel(payload.ts * 1000)}`;
    } else if (kind === 'command_ack') {
      touchDevice(id, { online: true, version: payload.version, releaseSeq: payload.release_seq });
      toast(`${payload.op || 'Command'}: ${payload.status || 'received'}`);
    }
  }

  function connect() {
    if (typeof mqtt === 'undefined') { toast('MQTT.js could not be loaded'); return; }
    if (!saved.user || !saved.password) {
      setConnection(false, 'Credentials required');
      $('settingsDialog').showModal();
      toast('Enter broker credentials to connect');
      return;
    }
    if (state.client) state.client.end(true);
    setConnection(false, 'Connecting...');
    state.client = mqtt.connect(saved.host, { username: saved.user, password: saved.password, clean: false, reconnectPeriod: 3000, connectTimeout: 8000, clientId: `sensor-console-${Math.random().toString(16).slice(2)}` });
    state.client.on('connect', () => { setConnection(true, 'Connected'); state.client.subscribe(topics, { qos: 1 }); toast('Connected to local broker'); });
    state.client.on('message', (topic, message) => handleMessage(topic, message.toString()));
    state.client.on('reconnect', () => setConnection(false, 'Reconnecting...'));
    state.client.on('close', () => setConnection(false, 'Disconnected'));
    state.client.on('error', (error) => {
      const message = error.message || 'MQTT connection error';
      setConnection(false, message.toLowerCase().includes('not authorized') ? 'Not authorised' : 'Connection error');
      if (message.toLowerCase().includes('not authorized') || message.toLowerCase().includes('not authorised')) {
        state.client.end(true);
        toast('Broker rejected credentials. Check username and password.');
      } else {
        toast(message);
      }
    });
  }

  function sendCommand() {
    const deviceId = $('deviceId').value.trim();
    const op = $('commandSelect').value;
    if (!deviceId) { toast('Choose a target device first'); $('deviceId').focus(); return; }
    if (!state.client?.connected) { toast('Connect to the broker first'); return; }
    const command = { command_id: `console-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`, op };
    if (op === 'set_interval') command.value = Number($('intervalValue').value);
    state.client.publish(`server/${deviceId}/commands`, JSON.stringify(command), { qos: 1, retain: false }, (error) => {
      if (error) { toast(`Publish failed: ${error.message}`); return; }
      state.commands += 1; $('commandCount').textContent = state.commands; toast(`${op} sent to ${deviceId}`); addActivity(`server/${deviceId}/commands`, JSON.stringify(command), 'SENT');
    });
  }

  $('settingsButton').addEventListener('click', () => $('settingsDialog').showModal());
  $('settingsForm').addEventListener('submit', (event) => { event.preventDefault(); saved.host = $('brokerHost').value.trim(); saved.user = $('mqttUser').value.trim(); saved.password = $('mqttPassword').value; localStorage.setItem('sensor-broker-host', saved.host); localStorage.setItem('sensor-broker-user', saved.user); localStorage.setItem('sensor-broker-password', saved.password); $('settingsDialog').close(); connect(); });
  $('sendButton').addEventListener('click', sendCommand);
  $('commandSelect').addEventListener('change', () => { $('intervalField').hidden = $('commandSelect').value !== 'set_interval'; });
  $('clearButton').addEventListener('click', () => { state.devices.clear(); state.messages = 0; $('messageCount').textContent = '0'; renderDevices(); });
  $('pauseButton').addEventListener('click', () => { state.paused = !state.paused; $('pauseButton').textContent = state.paused ? 'Resume' : 'Pause'; });
  renderDevices();
  connect();
})();
