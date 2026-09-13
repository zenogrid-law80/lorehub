# lorehub.zenogrid.co.kr TLS

`lorehub.zenogrid.co.kr`은 사설 주소를 가리키므로 Let's Encrypt의 HTTP-01 검증 대신 DNS-01 검증을 사용합니다. 인증서와 개인 키는 Git 저장소 밖의 Docker `letsencrypt` volume에 보관합니다.

## 최초 발급

LoreHub는 reverse proxy 뒤의 `127.0.0.1:8080`에서 실행합니다.

```dotenv
LOREHUB_PUBLIC_URL=https://lorehub.zenogrid.co.kr
LOREHUB_BIND=127.0.0.1:8080
```

다음 명령으로 실제 인증서 발급을 시작합니다.

```bash
docker compose --profile tls run --rm certbot \
  certonly --manual --preferred-challenges dns \
  --cert-name lorehub.zenogrid.co.kr \
  --key-type ecdsa --agree-tos --register-unsafely-without-email \
  -d lorehub.zenogrid.co.kr
```

Certbot이 값을 표시하면 Gabia DNS 관리 화면에 다음 TXT 레코드를 추가합니다.

```text
호스트: _acme-challenge.lorehub
종류: TXT
값: Certbot이 표시한 값
```

공인 DNS에서 같은 값이 보이는지 확인한 다음 Certbot 터미널에서 Enter를 누릅니다.

```bash
dig +short TXT _acme-challenge.lorehub.zenogrid.co.kr @1.1.1.1
```

발급이 끝나면 reverse proxy를 시작합니다.

```bash
docker compose --profile tls up -d reverse-proxy
curl -I https://lorehub.zenogrid.co.kr/
```

Google Cloud OAuth client의 authorized redirect URI는 다음 값이어야 합니다.

```text
https://lorehub.zenogrid.co.kr/auth/google/callback
```

## 갱신

Gabia DNS를 수동으로 변경하는 방식이라 자동 갱신되지 않습니다. 만료 전에 최초 발급 명령을 다시 실행하고 새 TXT 값을 등록합니다. 발급이 끝난 뒤 Nginx가 새 인증서를 읽도록 다시 로드합니다.

```bash
docker compose --profile tls exec reverse-proxy nginx -s reload
docker compose --profile repository restart lore-server
```

DNS API가 있는 공급자로 `_acme-challenge.lorehub.zenogrid.co.kr`을 CNAME 또는 NS 위임하면 갱신을 자동화할 수 있습니다.
