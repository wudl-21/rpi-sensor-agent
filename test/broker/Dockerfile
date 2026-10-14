FROM eclipse-mosquitto:2

USER root

COPY broker-entrypoint.sh /broker-entrypoint.sh
RUN chmod 0755 /broker-entrypoint.sh

ENTRYPOINT ["/broker-entrypoint.sh"]