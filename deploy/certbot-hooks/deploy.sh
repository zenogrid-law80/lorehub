#!/bin/sh
set -eu

reverse_proxy=$(docker ps -q --filter label=com.docker.compose.service=reverse-proxy)
[ -z "$reverse_proxy" ] || docker kill -s HUP $reverse_proxy >/dev/null
lore_server=$(docker ps -q --filter label=com.docker.compose.service=lore-server)
[ -z "$lore_server" ] || docker restart $lore_server >/dev/null
