# LoreHub

Lore VCS 기반 개발 플랫폼입니다. 기존 `lore-runner`의 CI 기능을 LoreHub 내부 runner로 통합했습니다. GitLab과 유사한 정보 구조의 웹 대시보드에서 현재 Lore 서버의 repository를 관리하고, 특정 Lore revision의 파이프라인을 생성해 상태, job, 로그를 확인하거나 취소할 수 있습니다. Axum coordinator가 실행 요청과 runner API를 제공하고, 워커는 이 API에서 파이프라인을 받아 Lore CLI로 소스를 내려받습니다. 실행 설정은 **요청한 revision에 포함된 `.lore-ci.toml`**에서 읽습니다.

Git checkout, GitLab API, Git commit SHA를 사용하지 않습니다. Lore의 64자리 revision hash를 사용합니다.

```text
사용자 ── Google 로그인 (`@zenogrid.co.kr`)
       │ 세션 쿠키 + CSRF token
       │ POST /api/v1/pipelines
       ▼
  API coordinator ───── PostgreSQL
       ▲
       │ HTTPS: claim / heartbeat / 상태 / 로그
 Rust worker × N
                            │
                  lore clone --revision HASH
                            │
                     .lore-ci.toml
                            │
                      /bin/sh -e
```

## 프로젝트 구조

```text
src/
  main.rs             CLI 인자와 프로세스 종료 신호
  server/             HTTP 서버, Google OIDC 인증과 API
  ci/                 파이프라인 설정, Postgres 큐·상태·로그
  runner/             워커와 shell executor
  vcs/lore.rs         Lore CLI 명령 구성
migrations/           기존 PostgreSQL 스키마
examples/             .lore-ci.toml 예제
web/                  내장 CI 대시보드 HTML, CSS, JavaScript
```

실행 파일과 Rust crate 이름은 `lorehub`입니다. `lorehub serve`가 서버를, `lorehub worker`가 실행 워커를 시작합니다. 두 프로세스는 같은 바이너리를 사용하고 역할별 모듈로 분리되어 있습니다. Lore 명령 인자는 `vcs`에서 구성하고, 자식 환경변수·타임아웃·프로세스 종료는 `runner`가 담당합니다.

## lore-runner에서 이전

| 기존 | 변경 |
| --- | --- |
| `lore-runner` 실행 파일 / `lore_runner` crate | `lorehub` |
| 정적 API bearer token | Google Workspace 로그인 세션 |
| `LORE_RUNNER_BIND` | `LOREHUB_BIND` |
| `LORE_RUNNER_WORK_DIR` | `LOREHUB_WORK_DIR` |
| `lore-runner-api.service` | `lorehub-api.service` |
| `lore-runner-worker@.service` | `lorehub-worker@.service` |
| `/etc/lore-runner/environment` | `/etc/lorehub/environment` |
| `RUST_LOG=lore_runner=info` | `RUST_LOG=lorehub=info` |

기존 배포에서는 환경 파일의 위 키와 서비스 실행 경로를 변경하세요. 서버에는 `DATABASE_URL`, `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `LOREHUB_PUBLIC_URL`이 필요하고 runner에서는 `DATABASE_URL`을 제거합니다. `LORE_BIN`, `/api/v1` 경로, `.lore-ci.toml`, job의 `LORE_*` 문맥 변수는 유지됩니다. 기존 job 스크립트를 위해 `LORE_RUNNER=true`도 계속 제공합니다. 이전 정적 token과 `LORE_RUNNER_*` 설정은 읽지 않습니다.

`0002_google_auth.sql` migration이 `users`, `oauth_states`, `sessions` 테이블을 추가합니다. 기존 pipeline·job·log 테이블과 데이터는 변경하지 않습니다. 새 Compose 기본 DB·계정은 `lorehub`입니다. **기존 Compose DB가 있다면 원래 DB·계정·volume을 유지하고 기존 연결 문자열을 사용하세요.** 프로젝트 디렉터리 이름이 바뀌면 Compose의 기본 volume 이름도 달라지므로 새 기본 설정으로 기존 데이터가 자동 이전되지는 않습니다. OS 계정을 변경하는 경우 Lore 인증과 작업 디렉터리 권한도 새 서비스 계정에 맞춰 설정합니다.

## 현재 제공하는 기능

- Google Workspace OIDC 로그인과 `@zenogrid.co.kr` 조직 제한
- 일반 사용자·관리자 계정 등급과 관리자 전용 등급 변경
- 관리자의 전체 계정 그룹·Sparse View 열람과 소유자 전용 수정·삭제
- 반응형 CI 대시보드, 상태·repository·branch·pipeline 필터와 검색, repository·branch 트리별 workspace 공용 pipeline graph, pipeline/job 상세와 실시간 로그
- 운영체제 설정을 따르는 시스템 테마와 라이트·다크 테마 선택
- runner 전체·online·offline 대수, 운영체제·아키텍처·버전·현재 작업 관리 화면
- 현재 Lore 서버의 repository 목록·검색·생성·URL 복사·삭제 관리 화면과 branch 기반 수동 파이프라인 실행
- repository별 자동 CI 감지 branch 선택; 기본 `main`, 미선택 시 자동 CI 일시 중지
- 과거 Lore branch ID를 현재 branch 이름으로 정규화해 실행 이력에 표시
- PostgreSQL 사용자·로그인 state·8시간 세션, HttpOnly/SameSite 쿠키와 CSRF 보호
- 로그인으로 보호한 파이프라인 생성·목록·상세·취소 API
- PostgreSQL `FOR UPDATE SKIP LOCKED` 기반 원자적 작업 할당
- 여러 워커의 파이프라인 동시 실행; 한 파이프라인의 job은 stage 순서로 순차 실행
- job 상태, 종료 코드, stdout/stderr와 cursor 기반 로그 조회
- job 타임아웃, 취소 시 프로세스 그룹 종료, 작업 디렉터리 정리
- 5초 heartbeat / 30초 lease, 만료된 워커의 작업 실패 처리
- 바이너리에 포함된 DB migration, 설정 검증 CLI, SIGINT/SIGTERM 처리

executor는 **신뢰할 수 있는 저장소를 위한 플랫폼 shell executor**입니다. Linux에서는 POSIX shell, Windows에서는 PowerShell로 실행됩니다. 스크립트는 워커 계정의 파일·네트워크 권한을 갖습니다. 전용 계정/VM에서 실행하세요. Runner에는 PostgreSQL 접속 정보가 없으며 Google OAuth 자격 증명도 자식 환경변수로 전달하지 않지만, 이것이 프로세스나 파일 접근 격리를 제공하는 것은 아닙니다.

## 실행

필요한 구성은 Rust 1.88 이상, PostgreSQL 18, Lore CLI 및 접근 가능한 Lore 서버입니다. 워커는 Linux, Windows와 macOS에서 동작합니다. Lore CLI 호출 형식은 로컬 `lore 0.9.0+783` 도움말과 [공식 CLI 문서](https://epicgames.github.io/lore/reference/lore-cli-commands/)를 기준으로 구현했습니다.

```bash
cd /Users/law80/GitHub/lorehub
cargo build --locked
docker compose up -d postgres
cp .env.example .env
```

Google Cloud Console에서 OAuth client 유형을 **Web application**으로 생성하고 authorized redirect URI에 다음 주소를 정확히 등록합니다.

```text
http://127.0.0.1:8080/auth/google/callback
```

운영 환경에서는 실제 HTTPS origin을 사용합니다. `.env`의 `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `LOREHUB_PUBLIC_URL`을 설정하세요. `LOREHUB_PUBLIC_URL`은 origin만 입력하며 경로, query, fragment를 포함하지 않습니다. 이 값이 `https`이면 인증 쿠키에도 `Secure`가 설정됩니다. `.env`는 자동 로딩하지 않으므로 각 터미널에서 다음과 같이 내보냅니다.

`lorehub.zenogrid.co.kr` 운영 환경의 Let's Encrypt DNS-01 발급과 Nginx TLS reverse proxy 구성은 [`deploy/TLS.md`](deploy/TLS.md)를 따릅니다. LoreHub는 `127.0.0.1:8080`에서 실행하고 reverse proxy만 80/443 포트를 엽니다.

```bash
set -a
. ./.env
set +a
```

첫 번째 터미널:

```bash
cargo run --locked -- serve
```

브라우저에서 `LOREHUB_PUBLIC_URL`의 루트(로컬 기본값 `http://127.0.0.1:8080/`)를 열면 대시보드가 표시됩니다. 프런트엔드 파일은 Rust 바이너리에 포함되므로 별도의 Node.js 빌드나 정적 파일 서버는 필요하지 않습니다.

두 번째 터미널:

```bash
cargo run --locked -- worker
```

Coordinator 시작 시 migration이 자동 적용됩니다. Worker는 `LOREHUB_PUBLIC_URL`의 coordinator runner API에만 연결하며 PostgreSQL에는 접근하지 않습니다. Coordinator를 먼저 시작한 뒤 워커 프로세스를 추가하면 서로 다른 파이프라인을 병렬 처리합니다. `worker --once`는 최대 한 건만 처리하고 종료하며, 큐가 비어 있어도 종료합니다. Runner는 시작할 때 Docker CLI 설치 여부를 자동 감지하며 Runner 목록의 Docker 열에 결과를 표시합니다.

Compose의 `runner` profile에는 Linux runner 1개가 포함되어 있습니다. LoreHub를 release 빌드하고 공식 Lore v0.9.0 Linux 바이너리를 체크섬 검증 후 포함하며 작업 공간과 Cargo 다운로드 캐시는 named volume에 유지됩니다. `deploy/secrets/`의 JWT 키·JWKS와 runner가 접근할 수 있는 `LOREHUB_PUBLIC_URL`을 준비해 실행합니다.

```bash
docker compose --profile runner up -d --build runner
docker compose logs -f runner
```

Docker runner에서 호스트 coordinator를 사용할 때 `LOREHUB_PUBLIC_URL`의 host는 `host.docker.internal`이어야 합니다. Runner 화면에는 기본 이름 `compose-linux-runner-01`로 등록되며 `LOREHUB_RUNNER_NAME`으로 변경할 수 있습니다.

## Lore repository server

Compose의 `repository` profile은 공식 Lore 소스의 `lore-server/Dockerfile`로 `loreserver` 이미지를 빌드합니다. 기본 소스 경로는 LoreHub 옆의 `../lore`이며 `LORE_SOURCE_DIR`로 변경할 수 있습니다. 데이터는 `lore_data` volume에 보존되고 QUIC은 LoreHub의 Let’s Encrypt 인증서를 사용합니다.

```bash
docker compose --profile repository up -d --build lore-server
curl -i http://127.0.0.1:41339/health_check
```

repository 주소는 `lores://127.0.0.1:41337/<repository>`입니다. Coordinator의 관리 API는 `LORE_SERVER_URL`에 지정한 서버를 `LORE_BIN` CLI로 관리합니다. 운영 화면에 공개 주소를 표시하려면 `LORE_SERVER_URL=lores://lorehub.zenogrid.co.kr:41337`처럼 설정합니다. `41337/TCP`와 `41337/UDP`는 기본적으로 loopback에만 공개됩니다. 신뢰할 수 있는 사내망 클라이언트가 직접 접속해야 할 때만 `.env`의 `LORE_SERVER_BIND_IP`를 서버의 사설 IP로 지정하고 컨테이너를 다시 생성합니다.

Apple Silicon에서는 공식 배포 지침에 따라 `linux/amd64` 이미지를 사용합니다. 네이티브 Linux ARM 서버에서 Graviton3 호환 빌드를 사용하는 경우에만 `LORE_SERVER_PLATFORM`을 `linux/arm64`로 변경하세요.

### S3 + DynamoDB Local 저장소

`repository` profile은 Lore의 내장 AWS store plugin을 사용합니다. immutable fragment payload는 `S3_BUCKET`의 실제 S3 bucket에 저장하고, fragment association/state, mutable repository 데이터와 lock은 `amazon/dynamodb-local:3.3.1` 컨테이너에 저장합니다. DynamoDB Local 데이터는 `dynamodb_data` volume에 유지되며 외부에는 `127.0.0.1:8000`으로만 노출됩니다. `dynamodb-init`이 다음 테이블을 idempotent하게 생성한 후 Lore 서버를 시작합니다.

- `lorehub-fragments`
- `lorehub-fragment-state`
- `lorehub-mutable`
- `lorehub-locks`

`.env`에 `S3_ENDPOINT`, `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`, `S3_REGION`을 설정합니다. AWS S3에 맞춰 path-style URL은 비활성화되어 있습니다. S3 호환 스토리지에서 path-style URL이 필요하면 `deploy/lore-server.toml`의 `s3_force_path_style`을 변경합니다. table 이름은 `LORE_DYNAMODB_*` 변수로 변경할 수 있습니다.

```bash
docker compose --profile repository up -d dynamodb-local dynamodb-init lore-server
docker compose --profile repository ps
curl --fail http://127.0.0.1:41339/health_check
```

이 구성에서 `dynamodb_data`는 S3 payload에 대한 association과 repository의 mutable state를 보관하므로 함께 백업해야 합니다. volume을 잃으면 S3 object만으로 repository를 복구할 수 없습니다. 이전 local `lore_data` volume은 자동 변환되지 않으며 Compose가 삭제하지 않고 보존합니다. 기존 local repository를 이전하려면 별도의 Lore store migration 또는 repository 단위 push가 필요합니다.

### Lore repository 인증

`deploy/lore-server.toml`은 LoreHub를 JWT issuer와 권한 서비스로 신뢰합니다. LoreHub는 Google Workspace 로그인을 통과한 `@zenogrid.co.kr` 사용자에게 RS256 Lore access token을 발급하고, Lore 서버는 `https://lorehub.zenogrid.co.kr/.well-known/jwks.json`의 공개키로 서명을 검증합니다. UCS 호환 gRPC API는 애플리케이션의 `127.0.0.1:8081`에서 실행되며 Nginx가 `epic_urc.UrcAuthApi`와 `ucs.auth.RebacApi` 경로만 HTTPS/2로 중계합니다. 개인키와 JWKS는 각각 `LORE_JWT_PRIVATE_KEY`, `LORE_JWT_JWKS` 파일에서 읽으며 `deploy/secrets/`는 Git에서 제외됩니다.

CLI 대화형 로그인은 Lore 서버가 공지하는 `https://lorehub.zenogrid.co.kr` UCS endpoint를 사용합니다. 다음 명령을 실행하면 브라우저에서 Google 로그인이 열리고, `@zenogrid.co.kr` 계정 인증이 끝난 뒤 CLI에 1시간짜리 사용자 token이 저장됩니다.

```bash
lore auth login lores://lorehub.zenogrid.co.kr:41337/test-project
```

로그인 후 Repository 화면의 **CLI access token** 버튼에서 1시간짜리 token을 발급합니다. token은 bearer credential이므로 다른 사람에게 전달하거나 shell history에 직접 입력하지 않습니다.

```bash
export LORE_ACCESS_TOKEN='발급받은-token'
lore \
  --identity-token "$LORE_ACCESS_TOKEN" \
  --access-token "$LORE_ACCESS_TOKEN" \
  repository list lores://lorehub.zenogrid.co.kr:41337
unset LORE_ACCESS_TOKEN
```

현재 Lore CLI의 repository service는 identity token을, repository 데이터 작업은 access token을 사용하므로 두 옵션에 같은 LoreHub JWT를 전달합니다. token은 `zenogrid.co.kr` audience와 현재 사용자가 접근 가능한 repository의 resource ID만 포함하며 만료 후 다시 발급해야 합니다. 일반 사용자는 본인 소유 리포지토리에만 접근할 수 있고, 관리자는 모든 리포지토리의 목록·조회·삭제와 파이프라인 생성·실행을 사용할 수 있습니다. 웹과 CLI 권한 검사는 DB의 현재 역할을 확인하므로 관리자 해제 후에는 기존 토큰으로도 다른 사용자 리포지토리에 접근할 수 없습니다. 역할 변경 후 CLI에서 새로 허용된 리포지토리를 보려면 토큰을 다시 발급하거나 로그인하세요. 웹 관리 API의 생성 작업에만 일회성 wildcard JWT를 내부에서 사용하고, 권한 서비스가 생성한 resource를 해당 Google 사용자에게 귀속시킵니다.

Lore 서버는 environment endpoint의 `auth_url`도 LoreHub로 공지하므로 QUIC 저장소 데이터 요청에 access token이 전달됩니다. 워커는 clone 직전에 5분짜리 JWT를 자동 발급해 `--identity-token`과 `--access-token`으로 넘깁니다. 따라서 OS 계정에 Lore 자격 증명을 별도로 저장할 필요가 없습니다. 워커에도 coordinator와 같은 `LOREHUB_PUBLIC_URL`, `LORE_JWT_PRIVATE_KEY`, `LORE_JWT_JWKS` 설정이 필요합니다. `LORE_BIN`은 `lore`처럼 PATH에서 찾는 이름 또는 절대 경로를 지정하세요. 워커의 PATH에는 빌드에 필요한 Rust 등의 도구가 있어야 합니다.

## 파이프라인 설정

Lore 저장소 루트에 `.lore-ci.toml`을 넣고 해당 revision을 Lore 서버에 push합니다. [Rust 프로젝트 예제](examples/.lore-ci.toml)를 참고하세요.

```toml
stages = ["test", "build"]

[[jobs]]
name = "unit-tests"
stage = "test"
script = ["cargo test --locked"]
timeout_seconds = 600

[[jobs]]
name = "release"
stage = "build"
script = ["cargo build --locked --release"]
timeout_seconds = 1800
```

```bash
cargo run --locked -- validate examples/.lore-ci.toml
```

stage는 선언 순서, 같은 stage의 job은 파일에 적힌 순서로 실행됩니다. 각 job은 별도의 `/bin/sh -e -c` 프로세스입니다. 한 job의 `script` 항목들은 같은 shell에서 실행되어 `cd`와 `export`가 유지됩니다. job 간에는 checkout 디렉터리를 공유하므로 빌드 결과 파일은 다음 job에서도 사용할 수 있습니다. 실패하면 뒤의 job은 `skipped`로 남습니다. POSIX `sh -e`의 일반적인 조건문·파이프라인 규칙이 적용되며 `pipefail`은 사용하지 않습니다.

job에는 `CI=true`, `LOREHUB=true`, `LORE_RUNNER=true`, `LORE_PIPELINE_ID`, `LORE_JOB_ID`, `LORE_JOB_NAME`, `LORE_REVISION`, `LORE_REPOSITORY_URL`, `LORE_BIN`, `LORE_PROJECT_DIR`과 자동 실행의 `LORE_BRANCH`가 전달됩니다. 인증된 Runner는 각 job 시작 시 저장소 범위의 `LORE_IDENTITY_TOKEN`과 `LORE_ACCESS_TOKEN`을 새로 발급하므로, 결과물을 commit/push하는 job은 `LORE_BIN` 실행 파일에 이 값을 전역 옵션으로 전달할 수 있습니다. 시스템 환경 중 `PATH`, `HOME`, `TMPDIR`, `LANG`, `LC_ALL`만 상속합니다.

설정은 최대 256 KiB, stage 32개, job 128개입니다. 이름에는 영문·숫자·`_`·`-`를 사용할 수 있습니다. job timeout은 기본 3,600초, 허용 범위는 1~86,400초이며 clone timeout은 300초입니다. 각 명령은 stdout/stderr 합계 1 MiB까지 저장하고 이후 출력은 버립니다. 파이프라인이 끝나면 작업 디렉터리를 제거합니다. 워커가 SIGKILL이나 호스트 장애로 종료되면 디렉터리가 남을 수 있습니다.

### 변경 경로별 자동 CI

저장소 루트의 `.lore-ci.toml`에 `[[pipelines]]`를 사용하면 push 자동 CI에 참여합니다. 기존 루트 `stages`/`jobs` 형식은 수동 실행용으로 유지하며 두 형식은 혼합할 수 없습니다. 전체 예제는 [monorepo.lore-ci.toml](examples/monorepo.lore-ci.toml)입니다.

Repository 화면의 **자동 CI Branch** 메뉴에서 hash 대신 remote branch 이름을 확인하고 `pipelines > changes` 감지 대상으로 사용할 branch를 선택합니다. 초기값은 `main`이며 여러 branch를 선택할 수 있습니다. 선택을 모두 해제하면 해당 repository의 자동 CI가 중지됩니다. 새로 선택한 branch는 현재 head를 기준점으로 저장하므로 설정 저장만으로 기존 변경분이 실행되지는 않으며, 이후 push부터 `changes` 규칙을 평가합니다. 제외한 branch의 cursor와 현재 routing graph는 제거하지만 기존 pipeline 실행 이력은 유지합니다.

```toml
[[pipelines]]
name = "client"
runner_os = "windows"
changes = ["Client/**"]
working_directory = "Client"
stages = ["check"]

[[pipelines.jobs]]
name = "client-check"
stage = "check"
script = ["./ci.ps1"]

[[pipelines]]
name = "server"
runner_os = "macos"
changes = ["Server/**"]
working_directory = "Server"
stages = ["test", "build"]

[[pipelines.jobs]]
name = "server-test"
stage = "test"
script = ["cargo test"]
```

Server 이미지를 macOS Runner에서 빌드하려면 같은 파이프라인에 build stage를 추가하고, `working_directory = "Server"`를 기준으로 Docker build context를 전달합니다.

```toml
[[pipelines.jobs]]
name = "docker-build"
stage = "build"
script = ["docker build --tag \"my-server:${LORE_REVISION}\" ."]
```

`runner_os`는 `windows`, `macos`, `linux` 중 하나입니다. `changes`는 저장소 루트 기준의 정확한 경로 또는 `디렉터리/**`를 받으며 대소문자를 구분합니다. 선행 `/`, `..`, 다른 glob 문법은 허용하지 않습니다. 삭제와 이동 전후 경로도 판단에 포함됩니다. 두 조건을 만족하면 두 파이프라인이 별도 큐에 들어가 각 OS에서 실행되며, 해당 OS의 Runner가 없으면 `queued`로 남습니다. 한 파이프라인 내부 stage/job 순서는 기존과 같습니다. `working_directory`는 checkout 내부 디렉터리여야 하며 외부를 가리키는 심볼릭 링크는 거부합니다.

Coordinator의 `serve`는 소유자가 등록된 Lore 저장소를 15초마다 확인하고 `lore notification subscribe`로 알림을 받습니다. 알림 수신 시 원격 branch head를 조회하며, 30초마다 추가 확인하고 4분마다 토큰·연결을 갱신합니다. 최초 실행은 기존 branch head를 기준점으로만 기록합니다. 이후 생성된 branch의 첫 push는 설정에 지정된 경로가 해당 revision에 존재하는지 확인해 실행합니다. 변경 비교, 요청 revision의 설정 읽기, 큐 생성이 성공한 경우에만 DB cursor를 갱신합니다. 실패하면 cursor를 유지해 다음 확인에서 재시도합니다. 연결 중단 중 여러 push가 쌓이면 마지막 처리 revision과 현재 head 사이의 최종 변경을 처리하며, 중간 push 각각을 재생하지 않습니다.

같은 저장소·branch·revision·파이프라인은 한 번만 생성됩니다. 설정 파일만 바꿔도 실행하려면 각 `changes`에 `.lore-ci.toml`을 명시하세요. 이름이 있는 파이프라인은 현재 자동 push 경로로 실행하며, 기존 수동 생성 API는 루트 `stages`/`jobs` 형식용입니다. UI 목록과 실행 그래프는 hash 대신 저장소 revision마다 고정된 숫자 표시 번호를 보여줍니다. 원본 64자리 hash는 실행 검증에 계속 사용됩니다. 실행 상세의 그래프는 일치한 폴더 규칙 → 파이프라인과 실행 경로 → 대상 또는 배정된 Runner → stage 순서와 현재 상태를 보여줍니다. 큐에 대기하는 동안에도 trigger가 저장한 설정 snapshot으로 예정 stage를 표시합니다.

적용 시 모든 Runner를 새 바이너리로 교체하고 coordinator를 재시작하세요. `lorehub migrate` 또는 새 프로세스 시작 시 push routing과 그래프용 migration이 적용됩니다. 이전 Runner의 OS를 무시한 claim도 DB에서 차단하지만, 새 설정을 실행하려면 Runner 업그레이드가 필요합니다. 설정은 반드시 실행 대상 Lore revision에 commit/push되어 있어야 합니다.

```bash
cargo run --locked -- validate examples/monorepo.lore-ci.toml
```

## Google 인증과 API

브라우저에서 `/auth/google/login`을 열어 로그인합니다. 요청에는 `hd=zenogrid.co.kr` 힌트를 보내고, 콜백에서는 Google 공개키로 ID token 서명과 `iss`, `aud`, `exp`, `nonce`를 검증합니다. 서명된 `hd`가 정확히 `zenogrid.co.kr`인지, 이메일이 검증되었고 실제 주소도 `@zenogrid.co.kr`인지 모두 확인합니다. 이메일 문자열 대신 변경되지 않는 Google `sub`를 사용자 식별자로 사용합니다.

서버의 `/`에는 별도 frontend 빌드 없이 바이너리에 포함된 LoreHub 대시보드가 제공됩니다. 로그인 후 Repository 메뉴에서 현재 Lore 서버의 목록·생성·URL 복사·삭제를 관리할 수 있습니다. Runners 메뉴에서는 등록 대수, 운영체제, 아키텍처, 버전, online/offline 상태와 현재 실행 중인 파이프라인을 확인합니다. Overview에서는 파이프라인 요약·검색·상태 필터와 생성·상세·job 로그·취소 기능을 사용할 수 있으며, 실행 중인 항목과 runner 상태는 5초 간격으로 갱신됩니다. Runners 화면에서는 Linux x86_64 tar.gz와 Windows x86_64 zip 설치 패키지도 다운로드할 수 있습니다. 두 패키지에는 LoreHub worker, Lore CLI, 환경 설정 예제와 OS별 서비스 설치 스크립트가 포함됩니다.

로그인 성공 시 8시간짜리 `lorehub_session` HttpOnly 쿠키와 `lorehub_csrf` 쿠키가 생성됩니다. 상태를 변경하는 `POST` 요청은 `lorehub_csrf` 쿠키 값을 `X-CSRF-Token` 헤더에도 담아야 합니다. 정적 bearer token 인증은 제공하지 않습니다. API는 기본적으로 `127.0.0.1:8080`에 바인딩하며 운영에서는 TLS reverse proxy를 사용하세요.

| Method | Path | 동작 |
| --- | --- | --- |
| GET | `/healthz` | DB 연결 확인: 200 / 503 |
| GET | `/auth/google/login` | Google 로그인 시작 |
| GET | `/auth/google/callback` | Google OAuth callback |
| POST | `/auth/logout` | 현재 세션 종료; CSRF 필요 |
| GET | `/api/v1/me` | 로그인 사용자 정보 |
| GET | `/api/v1/accounts` | 조직 계정과 계정 등급 목록 |
| POST | `/api/v1/accounts/{id}/role` | 관리자 전용 계정 등급 변경; CSRF 필요 |
| GET | `/api/v1/runners` | 등록 runner와 운영체제, heartbeat 상태, 현재 pipeline |
| GET | `/api/v1/pipeline-graphs` | 현재 revision의 폴더 trigger, 대상 runner와 stage 그래프 |
| GET | `/api/v1/runner-updates/{os}/{arch}` | Runner token으로 최신 실행 파일 manifest 조회 |
| GET | `/api/v1/runner-updates/{os}/{arch}/binary` | Runner token으로 검증 대상 실행 파일 다운로드 |
| GET | `/downloads/runners/linux-x86_64` | Linux x86_64 runner 설치 패키지 다운로드 |
| GET | `/downloads/runners/windows-x86_64` | Windows x86_64 runner 설치 패키지 다운로드 |
| GET | `/api/v1/repositories` | 현재 Lore 서버 주소와 repository 목록 |
| GET | `/api/v1/repositories/{name}/branches` | 활성 remote branch와 최신 revision 목록 |
| GET | `/api/v1/repositories/{name}/pipeline-branches` | 자동 CI 대상 branch 설정 조회 |
| POST | `/api/v1/repositories/{name}/pipeline-branches` | 자동 CI 대상 branch 설정 저장; CSRF 필요 |
| POST | `/api/v1/repositories` | repository 생성: 201; CSRF 필요 |
| POST | `/api/v1/repositories/{name}/delete` | repository 삭제: 204; CSRF 필요 |
| POST | `/api/v1/lore-token` | 로그인 사용자용 1시간 Lore JWT 발급; CSRF 필요 |
| POST | `/api/v1/pipelines` | 파이프라인 생성: 201 |
| GET | `/api/v1/pipelines` | 최근 100건. 기존 API 호환용 |
| GET | `/api/v1/pipeline-history?limit=100&before={pipeline_id}` | 최신순 실행 이력. 응답의 `next_before`를 다음 페이지의 `before`로 전달하며 최대 500건 |
| GET | `/api/v1/pipelines/{id}` | 파이프라인과 job 상태 |
| POST | `/api/v1/pipelines/{id}/cancel` | 실행 요청자 또는 관리자만 대기 작업을 취소하거나 실행 중 작업의 취소를 요청 |
| GET | `/api/v1/pipelines/{id}/logs?after=0&limit=100` | id 순서의 로그, 최대 500개 |

웹 UI에서는 repository와 branch를 선택하며 `main`이 있으면 기본으로 선택합니다. 서버가 branch의 최신 **64자리 revision hash**를 확인한 뒤 불변 revision으로 실행합니다. `repository_url`은 `LORE_SERVER_PUBLIC_URL`(미설정 시 `LORE_SERVER_URL`)과 등록된 repository 이름에 정확히 일치해야 하므로, 다른 Lore 서버를 실행 대상으로 지정할 수 없습니다. API에서 `branch`를 생략하면 기존처럼 Lore CLI의 `lore revision info` 등에서 얻은 전체 hash로 직접 요청할 수 있습니다.

웹 UI나 같은 origin의 JavaScript에서는 다음 방식으로 요청합니다.

```javascript
const csrf = document.cookie
  .split("; ")
  .find((entry) => entry.startsWith("lorehub_csrf="))
  ?.split("=")[1];

const response = await fetch("/api/v1/pipelines", {
  method: "POST",
  credentials: "same-origin",
  headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
  body: JSON.stringify({
    repository_url: "lores://127.0.0.1:41337/my-project",
    branch: "main",
    revision: "REPLACE_WITH_FULL_64_CHARACTER_LORE_HASH",
  }),
});
```

다음 로그 조회의 `after`에 마지막으로 받은 로그의 `id`를 넣습니다. `stdout`과 `stderr`는 각각의 순서를 유지하지만 두 stream 사이의 정확한 발생 순서는 보장하지 않습니다.

실행 중 취소는 보통 다음 heartbeat(5초 이내)에서 감지합니다. DB 장애나 lease 상실 시 워커는 실행을 중단하고, 살아 있는 coordinator/worker의 reaper가 만료 작업을 종료 상태로 바꿉니다. **자동 재시도는 하지 않습니다.** 배포 등의 외부 부수 효과는 취소나 실패로 되돌아가지 않습니다. 재실행하려면 새 파이프라인을 요청합니다.

## 검증

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
```

실제 PostgreSQL을 사용하는 통합 테스트는 명시적으로 실행합니다. SQLx가 테스트마다 별도 DB를 만들고 정리하므로 `CREATEDB` 권한이 있는 **테스트용 인스턴스**를 지정하세요.

```bash
DATABASE_URL=postgres://lorehub:lorehub@127.0.0.1:5432/lorehub \
  cargo test --test integration -- --ignored
```

통합 테스트는 원자적 claim, lease fencing, 취소 경쟁, 로그인 세션·CSRF·입력 검증·로그 cursor, stage 실행·실패 전파·정리, 타임아웃의 자식 프로세스 종료, 출력 제한을 검사합니다. 단위 테스트는 허용 도메인 판단을 검사합니다. 워커 테스트는 명령 인자를 검증하는 Lore CLI fixture를 사용하고 실제 shell과 PostgreSQL을 실행합니다. 실제 Google OAuth client 및 Lore 서버와의 end-to-end 검증은 별도로 필요합니다.

## 서비스 운영 및 다음 단계

`deploy/`의 systemd unit 예제를 사용할 수 있습니다. API coordinator 바이너리는 `/usr/local/bin/lorehub`에 설치하고, Runner 바이너리는 아래 설치 스크립트로 전용 state 디렉터리에 배치합니다. `lorehub` OS 계정과 `/etc/lorehub/environment` 환경 파일을 준비하고 워커 unit의 PATH를 설치된 빌드 도구 위치에 맞게 조정하세요. 환경 파일은 해당 서비스 관리자만 읽을 수 있게 설정합니다.

### Linux runner

Linux runner는 `/bin/sh -e`로 각 job을 실행하고 process group 전체를 종료하므로 timeout과 취소 시 자식 프로세스도 함께 정리됩니다.

```bash
cargo build --locked --release
sudo LOREHUB_RUNNER_INSTANCE=1 LOREHUB_BINARY=target/release/lorehub sh deploy/linux/install-runner.sh
sudo systemctl start lorehub-worker@1.service
```

`/etc/lorehub/environment`의 coordinator URL, Lore CLI 경로, JWT 파일 경로를 실제 값으로 변경해야 합니다. 여러 Linux runner는 `@1`, `@2`처럼 instance 번호를 달리해 실행합니다. 실행 파일은 `/var/lib/lorehub-1/bin/lorehub`처럼 Runner 전용 state 디렉터리에 설치되어 `lorehub` 서비스 계정이 검증된 업데이트를 교체할 수 있습니다.
`LOREHUB_RUNNER_NAME`으로 UI에 표시할 이름을 정할 수 있습니다. runner ID는 작업 디렉터리의 `.runner-id`에 자동 저장되므로 재시작해도 같은 장비로 표시됩니다. 작업 디렉터리를 교체하는 환경에서는 `LOREHUB_RUNNER_ID`에 고정 UUID를 지정할 수 있습니다.

### Windows runner

Windows runner는 Windows PowerShell에서 job을 실행합니다. timeout이나 취소가 발생하면 `taskkill /T /F`로 해당 job의 자식 프로세스 트리를 종료합니다. Runners 화면의 MSI를 관리자 권한으로 실행하면 Runner와 Lore CLI가 설치되고 `LoreHub Runner` 시작 작업이 등록됩니다. 설치 후 `C:\ProgramData\LoreHub\runner.env`와 JWT 파일을 설정하고 작업을 시작합니다.

```powershell
notepad C:\ProgramData\LoreHub\runner.env
Start-ScheduledTask -TaskName "LoreHub Runner"
```

Rust 서버를 SYSTEM runner에서 빌드할 때는 관리자 Command Prompt에서 설치 패키지의 `install-server-build-prerequisites.bat`를 한 번 실행하세요. 이 스크립트는 Visual Studio 2022 Build Tools의 C++ workload와 Windows SDK를 설치하고, Rust를 `C:\ProgramData\LoreHub\toolchains`에 설치합니다. `runner.env.example`의 `CARGO_HOME`, `RUSTUP_HOME`, `PATH` 항목을 `C:\ProgramData\LoreHub\runner.env`에 유지해야 SYSTEM job이 해당 도구를 찾습니다. Runner는 job 환경에 이 두 Rust 경로를 전달하고, `%PATH%`처럼 환경 변수를 포함한 `runner.env` 값을 확장합니다.

MSI를 직접 만들 때는 Windows x86_64 실행 파일을 준비한 뒤 `build-msi.sh`를 사용합니다. macOS 빌드 호스트에서는 `brew install msitools`가 필요합니다.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --locked --release --target x86_64-pc-windows-msvc
```

```sh
cargo build --locked --release --no-default-features --target x86_64-pc-windows-gnu --target-dir target/runner-dist-windows
deploy/windows/build-msi.sh 0.2.13 target/runner-dist-windows/x86_64-pc-windows-gnu/release/lorehub.exe path/to/lore.exe
```

MSI는 `C:\Program Files\LoreHub`에 실행 파일을 배치하고 `C:\ProgramData\LoreHub`의 설정·작업 디렉터리를 SYSTEM과 Administrators만 읽도록 제한합니다. 업그레이드와 제거 시 `ProgramData`의 설정, 키, Runner ID, 작업 데이터는 보존합니다.

### macOS runner

Runners 화면에서 Apple silicon용 `tar.gz` 패키지를 받은 뒤 압축을 풀고 설치합니다.

```sh
cargo build --locked --release --no-default-features --target-dir target/runner-dist-macos
deploy/macos/build-runner-package.sh 0.2.11 target/runner-dist-macos/release/lorehub path/to/lore
tar -xzf lorehub-runner-macos-aarch64-v0.2.11.tar.gz
sudo ./macos/install-runner.sh
sudo nano '/Library/Application Support/LoreHub Runner/runner.env'
sudo launchctl kickstart -k system/co.kr.zenogrid.lorehub.runner
```

설치기는 Runner와 Lore CLI를 `/usr/local/libexec/lorehub-runner`에 배치하고 시스템 시작 시 실행되는 LaunchDaemon을 등록합니다. 설정, 키, Runner ID, 작업 데이터는 `/Library/Application Support/LoreHub Runner`에 보존됩니다. 현재 배포 패키지는 Apple silicon(`aarch64`)용입니다.

### Runner 셀프 업데이트

일반 설치형 Runner는 유휴 상태에서 기본 5분마다 coordinator의 대상 OS/아키텍처 릴리스를 확인합니다. 현재 버전보다 높은 SemVer만 내려받으며, coordinator가 시작 시 계산한 파일 크기와 SHA-256이 모두 일치해야 설치합니다. job 실행 중에는 업데이트하지 않습니다. Unix Runner는 같은 디렉터리에서 실행 파일을 원자적으로 교체하고 종료 코드 `75`로 끝나 systemd/launchd가 다시 시작합니다. Windows Runner는 `lorehub.exe.update`로 준비한 뒤 `run-runner.ps1`이 프로세스 종료 후 교체하고 즉시 다시 실행합니다.

먼저 `Cargo.toml`의 package version을 새 SemVer로 올리고 각 플랫폼의 release 바이너리를 빌드한 뒤 coordinator 서버에서 다음처럼 같은 version 이름으로 게시합니다.

```bash
sudo sh deploy/publish-runner-release.sh macos aarch64 0.2.0 target/release/lorehub /var/lib/lorehub/runner-releases
sudo sh deploy/publish-runner-release.sh linux x86_64 0.2.0 target/x86_64-unknown-linux-gnu/release/lorehub /var/lib/lorehub/runner-releases
sudo sh deploy/publish-runner-release.sh windows x86_64 0.2.0 target/x86_64-pc-windows-msvc/release/lorehub.exe /var/lib/lorehub/runner-releases
sudo systemctl restart lorehub-api
```

Windows 셀프 업데이트에는 MSI가 아니라 raw x86_64 PE `lorehub.exe`를 게시해야 합니다. 게시 스크립트는 MSI나 다른 archive를 Windows 실행 파일로 전달하면 거부합니다.

CLI 기본 릴리스 디렉터리는 `deploy/runner-releases`이며 `LOREHUB_RUNNER_RELEASES_DIR`로 바꿀 수 있습니다. systemd API unit은 `/var/lib/lorehub/runner-releases`를 사용합니다. Runner에서는 `LOREHUB_AUTO_UPDATE=false`로 자동 업데이트를 끄거나 `LOREHUB_UPDATE_INTERVAL_SECONDS`로 확인 주기를 조정할 수 있습니다(최소 10초). 별도 배포 origin을 사용할 때만 `LOREHUB_RUNNER_UPDATE_URL`을 지정하며, 다운로드 URL은 manifest와 같은 HTTPS origin으로 제한됩니다. 컨테이너 Runner는 이미지 재빌드로 업데이트하므로 Compose 예제에서 자동 업데이트를 끕니다.

`.lore-ci.toml`의 `script`는 Linux/macOS에서 POSIX shell, Windows에서 PowerShell 문법으로 해석됩니다. 이름이 있는 자동 파이프라인은 `runner_os`로 해당 OS의 Runner만 선택합니다. 기존 루트 `stages`/`jobs` 형식에는 OS 조건이 없으므로 먼저 claim한 Runner에서 실행됩니다.

Runner는 JWT로 인증된 coordinator HTTP API를 통해 등록, claim, heartbeat, 상태 및 로그를 처리하며 PostgreSQL에 직접 연결하지 않습니다. 저장소 파일 탐색, OS 외의 runner tag, container executor, job DAG/병렬 실행, artifact/cache 업로드, secret 관리 및 보존 기간 정리는 아직 구현하지 않았습니다. 이 기능들은 각각 executor/트리거/스토리지 계층으로 확장할 수 있습니다.

### 계정, 계정 그룹, sparse workspace view

사이드바에서 **계정 관리** (`#accounts`), **계정 그룹 관리** (`#account-groups`),
**Workspace view 설정** (`#workspace-views`) 화면을 사용할 수 있습니다. 모바일에서는 페이지
선택 메뉴로 이동합니다. 기존 언어 설정(한국어·영어·중국어)을 따릅니다.

- 계정은 Google Workspace 최초 로그인 시 등록됩니다. 로그인한 조직 사용자는 계정 목록을
  조회하고 자신의 표시 이름을 수정할 수 있습니다. 표시 이름은 Google 재로그인 후에도 유지됩니다.
- 그룹 생성자는 이름·설명과 기존 계정의 구성원 목록을 관리합니다. 생성자는 항상 구성원으로
  포함되며, 그룹 목록에는 자신이 소유하거나 참여한 그룹만 표시됩니다.
- 그룹 소유자는 자신이 소유한 리포지토리에 대해 그룹별 view 프리셋을 저장할 수 있습니다.
  관리자라면 모든 리포지토리에서 본인 view 프리셋을 만들고 본인 그룹에 연결할 수 있습니다.
  그룹 구성원은 저장된 프리셋을 조회·다운로드할 수 있습니다. 그룹 가입은 Lore 리포지토리의
  접근 권한을 부여하지 않으며, 리포지토리 소유자 또는 관리자 권한 검사가 적용됩니다.
- 전체 workspace 모드는 모든 경로를 포함하는 빈 view 파일을 제공합니다. Sparse 모드는
  Lore view 원문을 순서대로 저장합니다. 일반 패턴은 제외하고 `!` 패턴은 포함하며, 뒤의
  규칙이 우선합니다. 예를 들어 다음 파일은 `client`와 `shared`를 포함하고 생성물을 제외합니다.

```text
**
!/client/
!/shared/
/client/generated/
```

저장한 설정의 **view 파일 다운로드** 버튼으로 받은 파일은 다음처럼 새 clone에 사용합니다.

```sh
lore repository clone --view ./view lores://lorehub.zenogrid.co.kr:41337/project-name
```

설정을 저장해도 기존 로컬 workspace나 CI runner에는 자동 적용되지 않습니다. 프리셋은
리포지토리 접근 제어나 서버의 경로 권한 정책이 아닙니다. 그룹 삭제 시 구성원 연결과 view
프리셋이 함께 삭제되며, 계정과 리포지토리는 유지됩니다.

`0009_account_groups.sql` migration은 표시 이름, 계정 그룹, 구성원, view 설정 저장 구조를
추가합니다. 새 바이너리를 배포한 뒤 기존 `lorehub migrate` 또는 coordinator 시작 시 적용됩니다.
변경 API는 로그인 세션과 CSRF 토큰을 요구하고, 소유권을 서버에서 확인합니다.

| API | 기능 |
| --- | --- |
| `GET /api/v1/accounts` | 조직 계정 목록 |
| `POST /api/v1/accounts/me` | 내 표시 이름 수정 (`name`) |
| `GET /api/v1/account-groups` | 소유·참여 그룹 목록 |
| `POST /api/v1/account-groups` | 그룹 생성 (`name`, `description`, `member_ids`) |
| `POST /api/v1/account-groups/{id}` | 그룹 수정 |
| `DELETE /api/v1/account-groups/{id}` | 그룹 삭제 |
| `GET /api/v1/workspace-repositories` | 프리셋을 관리할 수 있는 소유 리포지토리 목록 |
| `GET /api/v1/account-groups/{id}/views` | 그룹의 저장된 view 목록 |
| `POST /api/v1/account-groups/{id}/views/{resource_id}` | 프리셋 저장 (`mode`: `full` 또는 `sparse`, `rules`) |
| `DELETE /api/v1/account-groups/{id}/views/{resource_id}` | 프리셋 삭제 |

통합 테스트는 폐기 가능한 PostgreSQL 인스턴스에서 실행합니다.

```sh
DATABASE_URL=postgresql://... cargo test --test management -- --ignored
```
