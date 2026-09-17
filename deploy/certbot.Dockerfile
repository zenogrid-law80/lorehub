FROM certbot/certbot:v5.7.0

RUN apk add --no-cache curl jq docker-cli
