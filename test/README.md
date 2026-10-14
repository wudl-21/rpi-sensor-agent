# Full-install Docker test cluster

This environment simulates ten fresh Linux devices. Each device has its own root filesystem, state volume, SSH port, systemd PID 1, TLS CA, and automatically generated device ID.

## Prepare

From the repository root:

```bash
./test/prepare-test.sh

docker compose -f test/docker-compose.yml up -d --build
```

The test broker exposes MQTT TLS on host port `18884`, WebSockets on `9002`, and the OTA source on `8090`. Containers use the internal names `test-broker:8883` and `ota-server:80`.

## Install devices over SSH

Use the helper, one device at a time, or run the same commands manually inside the SSH session:

```bash
for port in $(seq 2221 2230); do
  ./test/install-device.sh "$port"
done
```

SSH credentials are `root` / `test-root-password`. The helper first installs `ca.crt`, runs `install.sh --no-start`, writes MQTT TLS and OTA settings, then enables both systemd services. No state file is mounted into the package, so each device creates its own ID on first start.

Verify IDs and logs:

```bash
for port in $(seq 2221 2230); do
  ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p "$port" root@127.0.0.1 'cat /var/lib/sensor-agent/state.json | grep device_id'
done

docker compose -f test/docker-compose.yml logs -f
```

The console can use `ws://localhost:9002/mqtt` with the test broker account `sensor-agent` / `test-password`.

## OTA test

Prepare a newer binary and manifest:

```bash
./test/prepare-ota.sh 0.2.0 2
```

Send `trigger_ota` to one device from the console. It should download from the Nginx OTA service, validate SHA-256, exit with code 75, and be restarted by its systemd unit.

## Network deployment / gray rollout

Build or prepare a release, then copy the three release files and the `.ready` marker into one device's incoming directory. For example:

```bash
docker cp deploy-artifacts/sensor-agent sensor-test-device-01:/var/lib/sensor-agent/incoming/release.gray.sensor-agent
docker cp deploy-artifacts/sensor-agent-updater sensor-test-device-01:/var/lib/sensor-agent/incoming/release.gray.sensor-agent-updater
docker cp deploy-artifacts/release.json sensor-test-device-01:/var/lib/sensor-agent/incoming/release.gray.json
docker exec sensor-test-device-01 sh -c 'printf gray > /var/lib/sensor-agent/incoming/release.gray.ready'
```

Upload `.ready` last. Confirm device 01 is healthy, then repeat for devices 02 through 10.

## Cleanup

```bash
docker compose -f test/docker-compose.yml down -v
rm -rf test/package test/certs test/broker/passwd test/ota/releases/*
```
