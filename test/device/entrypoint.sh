#!/bin/sh
set -eu

ssh-keygen -A
mkdir -p /run/sshd
exec /sbin/init
