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
- 반응형 CI 대시보드, 상태·repository·branch·pipeline 필터와 검색, 접근 가능한 repository·branch 트리별 pipeline graph, pipeline/job 상세와 실시간 로그
- 운영체제 설정을 따르는 시스템 테마와 라이트·다크 테마 선택
- runner 전체·online·offline 대수, 운영체제·아키텍처·버전·현재 작업 관리 화면
- 현재 Lore 서버의 repository 목록·검색·생성·URL 복사·삭제 관리, branch별 `.lore-ci.toml` 조회와 Visual/TOML 그래프 편집 화면, branch 기반 수동 파이프라인 실행
- repository별 자동 CI 감지 branch 선택; 기본 `main`, 미선택 시 자동 CI 일시 중지
- 과거 Lore branch ID를 현재 branch 이름으로 정규화해 실행 이력에 표시
- PostgreSQL 사용자·로그인 state·8시간 세션, HttpOnly/SameSite 쿠키와 CSRF 보호
- 로그인으로 보호한 파이프라인 생성·목록·상세·취소 API
- PostgreSQL `FOR UPDATE SKIP LOCKED` 기반 원자적 작업 할당
- 여러 워커의 파이프라인 동시 실행; 한 파이프라인의 job은 stage 순서로 순차 실행
- job 상태, 종료 코드, stdout/stderr와 cursor 기반 로그 조회
- job 타임아웃, 취소 시 프로세스 그룹 종료, 작업 디렉터리 정리
- 5초 heartbeat / 30초 lease, 만료된 워커의 작업 실패 처리
- Runner 등록·작업 수신·완료 보고의 제한된 재시도와 요청 ID 기반 작업 배정 중복 방지
- 바이너리에 포함된 DB migration, 설정 검증 CLI, SIGINT/SIGTERM 처리

executor는 **신뢰할 수 있는 저장소를 위한 플랫폼 shell executor**입니다. Linux에서는 POSIX shell, Windows에서는 PowerShell로 실행됩니다. 스크립트는 워커 계정의 파일·네트워크 권한을 갖습니다. 전용 계정/VM에서 실행하세요. Runner에는 PostgreSQL 접속 정보가 없으며 Google OAuth 자격 증명도 자식 환경변수로 전달하지 않지만, 이것이 프로세스나 파일 접근 격리를 제공하는 것은 아닙니다.

## 저장소 폴더 보기

사이드바 또는 저장소 카드의 **폴더 보기** (`#repository-tree`)에서 저장소와 브랜치를 선택합니다.
폴더 이름이나 화살표를 클릭하면 바로 아래 파일·폴더가 들여쓰기된 트리로 펼쳐지고 다시 클릭하면 접힙니다.
여러 폴더를 동시에 펼칠 수 있으며, 링크 폴더 이름 옆에는 **LINK** 배지를 표시합니다.
펼칠 때 하위 노드를 조회하고, 빈 폴더·로딩·오류 및 재시도는 해당 폴더 아래에 표시합니다.
조회는 선택한 브랜치의 revision에 고정되며 링크 내부는 원본의 고정 revision을 따릅니다.
새로고침하면 브랜치 정보를 다시 불러옵니다. 링크 원본에 접근할 수 없으면 오류를 표시합니다.

`GET /api/v1/repositories/{name}/tree?revision=HASH&path=PATH`는 로그인과 저장소 접근 권한을
확인하고 바로 아래 노드의 `name`, `kind`, `is_link`를 반환합니다. 루트는 빈 `path`입니다.
Lore CLI의 bare clone과 깊이가 제한된 JSON tree 조회를 사용하며 파일 본문은 내려받지 않습니다.
DB migration은 없으며 웹 자산과 API가 바이너리에 포함되므로 Coordinator 재빌드·배포가 필요합니다.

검증: `node --test tests/repository-tree.test.mjs tests/repository-context.test.mjs`,
`cargo test --locked --no-default-features --lib server::repositories`.
로컬 화면 확인: `node tests/web-links-preview.mjs` 실행 후 `http://127.0.0.1:4179/#repository-tree`.

## CI 설정 시각화

`CI 설정`에서 저장소와 branch를 선택하면 **전체 파이프라인** 의존성 개요가 기본으로 표시됩니다. 카드를 선택하면 설정을 확인할 수 있고, **단계·작업**으로 전환하면 기존 stage/job 편집기를 사용합니다. 연결선은 `needs` 관계이며, 한 파이프라인 안의 작업은 병렬이 아닌 순차 실행입니다.

**변경 경로 미리보기**에 저장소 기준 파일 경로를 한 줄씩 입력하면 일치한 파이프라인·규칙·파일을 표시합니다. 실제 push 처리와 동일하게 대소문자를 구분하고, 정확한 경로 또는 `directory/**` 규칙만 지원합니다. `needs`에 포함됐지만 경로가 일치하지 않는 파이프라인은 “의존성만 있음”으로 구분하며 자동 실행 대상으로 간주하지 않습니다. 실제 실행 여부에는 branch 자동 CI 정책과 Runner 상태도 영향을 줍니다.

편집 중 서버의 실제 설정 검증기를 사용해 첫 오류를 표시하고, 오류를 누르면 해당 파이프라인·단계·작업 입력란 또는 TOML 위치로 이동합니다. 저장 전 다시 검증하며, 분석/미리보기는 commit·push·파이프라인 실행을 하지 않습니다. 추가 DB migration은 없습니다.

그래프의 `＋`/`−`, **원래 크기**, **화면 맞춤**으로 배율을 조절하고 빈 공간을 드래그해 이동할 수 있습니다. **실행 취소/다시 실행**은 입력 변경, 작업·단계·파이프라인 삭제, 의존성 수정과 Visual/TOML 전환을 복원합니다. 단축키는 `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`이며 Windows에서는 `Ctrl+Y`도 지원합니다. 이력은 편집 중에만 유지되고 저장·취소·저장소 전환 시 초기화됩니다. 최근 최대 100개 상태를 보관하며 큰 설정에서는 메모리 한도에 맞춰 오래된 상태부터 제거합니다.

**변경 비교**는 저장된 원문과 실제 저장할 내용을 줄 단위로 보여 줍니다. Visual 편집에서 다시 작성되는 주석·서식도 포함합니다. 큰 변경은 나누어 표시하고 매우 긴 파일은 전체 원문 비교로 전환합니다. 이력·diff 회귀 테스트는 `node --test tests/ci-editor.test.mjs`로 실행합니다.

운영 데이터 없이 화면을 검증하려면 `cargo run --no-default-features --example ci_visual_preview` 실행 후 `http://127.0.0.1:4180/#ci-settings`를 엽니다. 이 테스트 서버는 localhost에만 바인딩하고, 저장은 메모리에만 반영합니다.

## 실행 그래프

실행 그래프는 최근 실행의 스냅샷과 **현재 설정**을 분리합니다. 과거 실행의 OS·경로·단계는 최신 설정으로 덮어쓰지 않습니다. 스냅샷이 없으면 기록된 작업만 표시하며, 스냅샷과 실제 등록된 작업이 다르면 실제 작업 기준임을 안내합니다.

단계별 작업 카드를 선택하면 상태·소요 시간·종료 코드와 해당 작업의 로그를 확인할 수 있습니다. 실패 위치 이동, 실행 중 작업 따라가기, 완료 단계 접기, 확대/축소와 화면 맞춤을 지원하며 자동 갱신 시 선택·배율·스크롤을 유지합니다. 로그는 작업별로 페이지 조회하고, 메모리에는 최근 불러온 내용만 제한적으로 보관합니다.

대기 사유는 서버의 claim 조건에 맞춰 선행 파이프라인 대기와 Runner 할당 대기를 구분합니다. 같은 저장소·branch·revision의 실제 선행 실행만 확인하며, 실행되지 않은 선행 파이프라인을 임의의 대기 원인으로 표시하지 않습니다. 알 수 없는 사유는 확인 중으로 표시합니다. DB migration은 추가하지 않습니다.

회귀 테스트: `node --test tests/execution-graph.test.mjs`. 운영 데이터 없이 UI를 확인하려면 `node tests/execution-preview.mjs` 실행 후 `http://127.0.0.1:4181/#graphs`를 엽니다.

**실행 분석**을 펼치면 다음 정보를 조회합니다. 이 조회는 읽기 전용이며 분석을 닫아 둔 실행에는 추가 조회를 하지 않습니다.

- **파이프라인 연결:** 같은 저장소·branch·revision의 최근 선행/후행 실행을 표시하고 상세 화면으로 이동합니다. 연결은 `needs`에 따른 관계이며, 실제 실행이 사용한 선행 실행 ID는 저장되어 있지 않으므로 과거의 확정된 실행 연결을 뜻하지 않습니다. 없는 선행 실행은 대기 중으로 추정하지 않습니다. 방향별 최대 100개를 표시하고 생략 여부를 안내합니다.
- **시간 타임라인:** 파이프라인 생성→시작의 대기와 시작→완료의 실행 시간을 구분합니다. 작업 시작 전 지연은 준비 및 이전 작업 시간을 포함하므로 별도의 큐 대기로 합산하면 안 됩니다. 진행 중인 작업은 서버 조회 시각을 기준으로 표시하며 잘못되거나 없는 시간은 추정하지 않습니다.
- **이전 실행 비교:** 같은 저장소·branch·파이프라인에서 이번 실행 제출 전에 완료된 직전 실행을 기준으로 삼습니다. OS·작업 디렉터리·Sparse View·작업 구성이 다르거나 스냅샷이 없으면 증감을 계산하지 않습니다. 조건이 맞아도 같은 단계/이름의 양쪽 성공 완료 작업만 비교하며, Revision·Runner 차이를 별도로 안내합니다. 소스·스크립트·부하가 다를 수 있으므로 참고용 시간 비교이지 성능 저하 판정은 아닙니다.

분석 API는 `GET /api/v1/pipelines/{id}/insights`이며, 인증된 기존 실행 조회와 동일한 접근 범위를 사용합니다. 여러 조회는 읽기 전용 repeatable-read transaction 안에서 일관된 스냅샷으로 처리합니다. 추가 회귀 테스트: `node --test tests/execution-analysis.test.mjs`.

### 실행 정보 조회 권한

목록·실행 이력·그래프·상세·작업 로그·실행 분석은 모두 현재 저장소 접근 권한을 따릅니다. 저장소 소유자, 해당 저장소에 접근 권한을 부여받은 계정 그룹의 소유자·구성원, 관리자가 조회할 수 있습니다. 그룹별 Sparse View 프리셋만 저장하는 것은 접근 권한 부여가 아닙니다. 그룹 구성원 제거·저장소 접근 권한 회수·관리자 해제는 기존 로그인 세션의 다음 조회부터 반영됩니다.

목록과 페이지 커서는 권한 필터를 적용한 실행만 대상으로 합니다. 접근할 수 없는 실행의 상세·로그·분석·취소는 없는 실행과 동일한 `404`를 반환하고, 접근할 수 없는 이력 커서는 알 수 없는 커서와 동일한 `400`을 반환합니다. 취소에는 저장소 접근 권한과 함께 기존 실행 요청자 또는 관리자 조건도 필요합니다. 분석의 선행·후행·이전 실행과 그래프의 최근 실행도 같은 조회 범위로 제한됩니다. Runner 목록은 조직에 공유하되 접근할 수 없는 현재 실행 ID는 표시하지 않습니다.

기존 실행에는 저장소 resource ID가 없으므로 **설정된 공개 서버 URL·storage backend·저장소 이름이 정확히 일치하고, 현재 저장소 등록 이후에 생성된 실행**만 연결합니다. 삭제된 저장소, 비활성화된 backend, 다른 서버 URL, 같은 이름으로 재생성되기 전의 실행은 관리자에게도 노출하지 않습니다. 공개 URL을 변경했거나 저장소를 나중에 재등록한 경우 과거 이력이 숨겨질 수 있으며, 이름만으로 자동 재연결하지 않습니다. DB migration은 추가하지 않습니다.

권한 회귀 테스트는 테스트용 PostgreSQL에서 실행합니다.

```sh
sh scripts/with-test-postgres.sh cargo test --locked --no-default-features --test pipeline_access -- --ignored
```

## 저장소 링크 관리

**Source → Root** 방향으로 연결합니다. 실제 링크와 pin은 Lore revision이 기준이며, DB에는 동기화 정책, 마지막 성공·오류, 생성 작업 이력을 별도로 저장합니다.

- 마지막으로 선택한 저장소·브랜치를 브라우저별로 기억하고, 저장된 선택이 없으면 링크가 감지된 저장소를 우선 선택합니다. 요약 목록은 watcher 인덱스 기준이며 다음 감지 주기까지 지연될 수 있습니다.
- 기본 **자동 동기화**는 Source push 알림 및 30초 보정 주기로 변경을 감지해 Root에 commit·push합니다. `main`뿐 아니라 각 remote branch를 인덱싱합니다. 자동 CI branch 설정과 별개이며, 순환 의존성이나 접근 권한 문제로 실패하면 오류를 표시합니다.
- **수동 동기화**에서는 사용자가 최신 revision으로 갱신할 때만 Root를 변경합니다. 정책 전환 자체는 Lore commit을 만들지 않습니다. 기존 링크는 정책이 없으면 자동 동기화를 유지합니다.
- 없는 Source 폴더 생성은 기본 활성화됩니다. 폴더 생성·commit·push 후 Root 연결에 실패해도 Source commit은 되돌리지 않습니다. 생성 이력의 **Root 링크 재시도**로 최신 Root revision을 확인하고 연결을 마무리합니다.
- 작업 ID로 중복 생성을 방지하고 검증 → Source 준비 → Root 링크 생성 → 완료 단계를 저장합니다. coordinator 중단으로 진행 중인 기록이 남으면 마지막 진행 기록으로부터 30분 후 재시도할 수 있습니다.
- 고급 옵션인 **Disable linked branch creation**은 Lore 브랜치 생성 동작이며 자동 동기화 정책과 별개입니다.

새 coordinator 시작 시 `0024_repository_link_management.sql`이 자동 적용됩니다. 기존 링크 정의를 변경하는 데이터 migration은 없습니다.

추가 API(로그인 필요, POST는 CSRF 필요):

| 경로 | 용도 |
| --- | --- |
| `GET /api/v1/repository-links/summary` | 접근 가능한 Root/branch별 링크 수 |
| `POST /api/v1/repositories/{name}/links/policy` | `branch`, `path`, `expected_revision`, `auto_update` 변경 |
| `GET /api/v1/repositories/{name}/link-operations?branch=main` | 최근 생성 작업 30건 |
| `POST /api/v1/repositories/{name}/link-operations/{id}/retry` | 실패·부분 실패 작업 재시도 |

기존 링크 생성 API에 `operation_id`(UUID), `create_source_directory`(기본 true), `auto_update`(기본 true)가 추가됩니다. 성공 응답의 `revision`, `source_path_created`는 유지하며 작업 상태·ID가 함께 반환됩니다. 실패·부분 실패는 HTTP 409와 저장된 오류, 중복 요청이 아직 실행 중이면 HTTP 202를 반환합니다. 권한·입력 검증 오류는 기존 4xx 응답을 사용합니다.

UI 단독 검증은 `node tests/web-links-preview.mjs` 실행 후 `http://127.0.0.1:4179/#repository-links`에서 가능합니다. 이 fixture는 운영 DB나 Lore에 연결하지 않으며 생성 실패 → 재시도 흐름을 메모리에서 재현합니다.

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

Compose의 `repository` profile은 공식 Lore 소스의 `lore-server/Dockerfile`로 `loreserver` 이미지를 빌드합니다. 기본 소스 경로는 LoreHub 옆의 `../lore`이며 `LORE_SOURCE_DIR`로 변경할 수 있습니다. `lore-server`는 DynamoDB + S3 backend를, `lore-server-local`은 `lore_data` volume의 Local File backend를 사용하며 두 서버 모두 LoreHub의 Let’s Encrypt 인증서를 사용합니다.

```bash
docker compose --profile repository up -d --build lore-server lore-server-local
curl -i http://127.0.0.1:41339/health_check
curl -i http://127.0.0.1:41340/health_check
```

repository 주소는 DynamoDB + S3가 `lores://127.0.0.1:41337/<repository>`, Local File이 `lores://127.0.0.1:41338/<repository>`입니다. Coordinator에는 각각 `LORE_SERVER_URL`/`LORE_SERVER_PUBLIC_URL`과 `LORE_LOCAL_SERVER_URL`/`LORE_LOCAL_SERVER_PUBLIC_URL`로 설정합니다. 예를 들어 운영 공개 주소는 `lores://lorehub.zenogrid.co.kr:41337`과 `lores://lorehub.zenogrid.co.kr:41338`입니다. Local File 선택을 활성화하려면 `LORE_LOCAL_SERVER_URL`을 반드시 설정해야 합니다. `41337`과 `41338`의 TCP/UDP는 기본적으로 loopback에만 공개됩니다. 신뢰할 수 있는 사내망 클라이언트가 직접 접속해야 할 때만 `.env`의 `LORE_SERVER_BIND_IP`를 서버의 사설 IP로 지정하고 컨테이너를 다시 생성합니다.

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

현재 Lore CLI의 repository service는 identity token을, repository 데이터 작업은 access token을 사용하므로 두 옵션에 같은 LoreHub JWT를 전달합니다. token은 `zenogrid.co.kr` audience와 현재 사용자가 접근 가능한 repository의 resource ID만 포함하며 만료 후 다시 발급해야 합니다. 일반 사용자는 본인 소유 리포지토리와 계정 그룹을 통해 명시적으로 접근 권한을 부여받은 리포지토리에 접근할 수 있고, 관리자는 모든 리포지토리의 목록·조회·삭제와 파이프라인 생성·실행을 사용할 수 있습니다. 웹과 CLI 권한 검사는 DB의 현재 역할을 확인하므로 관리자 해제 후에는 기존 토큰으로도 다른 사용자 리포지토리에 접근할 수 없습니다. 역할 변경 후 CLI에서 새로 허용된 리포지토리를 보려면 토큰을 다시 발급하거나 로그인하세요. 웹 관리 API의 생성 작업에만 일회성 wildcard JWT를 내부에서 사용하고, 권한 서비스가 생성한 resource를 해당 Google 사용자에게 귀속시킵니다.

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
needs = ["unit-tests"]
script = ["cargo build --locked --release"]
timeout_seconds = 1800
```

```bash
cargo run --locked -- validate examples/.lore-ci.toml
```

stage는 선언 순서로 실행됩니다. `needs`에는 먼저 완료되어야 하는 job 이름을 지정하며, 같은 stage의 job도 의존 순서에 맞춰 실행됩니다. 의존성은 이전 또는 같은 stage만 가리킬 수 있고 알 수 없는 job, 자기 자신, 중복 및 순환 참조는 거부됩니다. `needs`가 없는 같은 stage의 job은 파일에 적힌 순서를 유지합니다. 각 job은 별도의 `/bin/sh -e -c` 프로세스입니다. 한 job의 `script` 항목들은 같은 shell에서 실행되어 `cd`와 `export`가 유지됩니다. job 간에는 checkout 디렉터리를 공유하므로 빌드 결과 파일은 다음 job에서도 사용할 수 있습니다. 실패하면 뒤의 job은 `skipped`로 남습니다. POSIX `sh -e`의 일반적인 조건문·파이프라인 규칙이 적용되며 `pipefail`은 사용하지 않습니다.

job에는 `CI=true`, `LOREHUB=true`, `LORE_RUNNER=true`, `LORE_PIPELINE_ID`, `LORE_JOB_ID`, `LORE_JOB_NAME`, `LORE_REVISION`, `LORE_REPOSITORY_URL`, `LORE_BIN`, `LORE_PROJECT_DIR`과 자동 실행의 `LORE_BRANCH`가 전달됩니다. 인증된 Runner는 각 job 시작 시 저장소 범위의 `LORE_IDENTITY_TOKEN`과 `LORE_ACCESS_TOKEN`을 새로 발급하므로, 결과물을 commit/push하는 job은 `LORE_BIN` 실행 파일에 이 값을 전역 옵션으로 전달할 수 있습니다. 시스템 환경 중 `PATH`, `HOME`, `TMPDIR`, `LANG`, `LC_ALL`만 상속합니다.

설정은 최대 256 KiB, stage 32개, job 128개입니다. 이름에는 영문·숫자·`_`·`-`를 사용할 수 있습니다. job timeout은 기본 3,600초, 허용 범위는 1~86,400초이며 clone timeout은 300초입니다. 각 명령은 stdout/stderr 합계 1 MiB까지 저장하고 이후 출력은 버립니다. 파이프라인이 끝나면 작업 디렉터리를 제거합니다. 워커가 SIGKILL이나 호스트 장애로 종료되면 디렉터리가 남을 수 있습니다.

### 변경 경로별 자동 CI

저장소 루트의 `.lore-ci.toml`에 `[[pipelines]]`를 사용하면 push 자동 CI에 참여합니다. 기존 루트 `stages`/`jobs` 형식은 수동 실행용으로 유지하며 두 형식은 혼합할 수 없습니다. 전체 예제는 [monorepo.lore-ci.toml](examples/monorepo.lore-ci.toml)입니다.

Repository 화면의 **자동 CI Branch** 메뉴에서 hash 대신 remote branch 이름을 확인하고 `pipelines > changes` 감지 대상으로 사용할 branch를 선택합니다. 초기값은 `main`이며 여러 branch를 선택할 수 있습니다. 선택을 모두 해제하면 해당 repository의 자동 CI가 중지됩니다. 새로 선택한 branch는 현재 head를 기준점으로 저장하므로 설정 저장만으로 기존 변경분이 실행되지는 않으며, 이후 push부터 `changes` 규칙을 평가합니다. 제외한 branch의 cursor와 현재 routing graph는 제거하지만 기존 pipeline 실행 이력은 유지합니다. `.lore-ci.toml` 확인과 편집은 별도의 **CI 설정** (`#ci-settings`) 화면에서 repository와 branch를 선택해 수행합니다. Repository 목록의 **CI 설정** 버튼을 사용하면 해당 repository가 자동으로 선택됩니다.

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

파이프라인 간 실행 순서는 `needs`로 지정합니다. 아래는 관련 필드만 표시한 예입니다. `server-windows-build`와 `server`는 같은 저장소·branch·revision의 `data-table-generate` 실행이 큐에 있거나 실행 중이면 기다리고, 성공한 후에만 Runner가 claim할 수 있습니다. 의존성이 설정되고 branch를 아는 후속 파이프라인은 선행 파이프라인이 같은 branch에 게시한 결과를 포함하도록 실행 시작 시 최신 branch revision을 clone합니다. 원래 트리거 revision은 `LORE_REVISION`에 유지됩니다. 해당 선행 실행이 없으면 기존처럼 바로 실행하며, 선행 실행이 실패하거나 취소되면 후속 파이프라인도 실패 처리됩니다. 알 수 없는 파이프라인, 자기 자신, 중복 및 순환 의존성은 설정 검증에서 거부됩니다.

```toml
[[pipelines]]
name = "server-windows-build"
needs = ["data-table-generate"]

[[pipelines]]
name = "server"
needs = ["data-table-generate"]
```

Coordinator의 `serve`는 소유자가 등록된 Lore 저장소를 15초마다 확인하고 `lore notification subscribe`로 알림을 받습니다. 알림 수신 시 원격 branch head를 조회하며, 30초마다 추가 확인하고 4분마다 토큰·연결을 갱신합니다. 최초 실행은 기존 branch head를 기준점으로만 기록합니다. 이후 생성된 branch의 첫 push는 설정에 지정된 경로가 해당 revision에 존재하는지 확인해 실행합니다. 변경 비교, 요청 revision의 설정 읽기, 큐 생성이 성공한 경우에만 DB cursor를 갱신합니다. 실패하면 cursor를 유지해 다음 확인에서 재시도합니다. 연결 중단 중 여러 push가 쌓이면 마지막 처리 revision과 현재 head 사이의 최종 변경을 처리하며, 중간 push 각각을 재생하지 않습니다.

같은 저장소·branch·revision·파이프라인은 한 번만 생성됩니다. 설정 파일만 바꿔도 실행하려면 각 `changes`에 `.lore-ci.toml`을 명시하세요. 이름이 있는 파이프라인은 현재 자동 push 경로로 실행하며, 기존 수동 생성 API는 루트 `stages`/`jobs` 형식용입니다. UI 목록과 실행 그래프는 hash 대신 저장소 revision마다 고정된 숫자 표시 번호를 보여줍니다. 원본 64자리 hash는 실행 검증에 계속 사용됩니다. 실행 상세의 그래프는 일치한 폴더 규칙 → 파이프라인과 실행 경로 → 대상 또는 배정된 Runner → stage 순서와 현재 상태를 보여줍니다. 큐에 대기하는 동안에도 trigger가 저장한 설정 snapshot으로 예정 stage를 표시합니다.

### Lore link 자동 갱신

Coordinator는 각 저장소 `main` branch의 `lore link list` 결과를 역방향 의존성으로 저장합니다. Link 저장소의 해당 branch가 push되면 이를 참조하는 root 저장소를 찾아 최신 root head를 다시 확인한 뒤 `lore link update`, `lore commit`, `lore push`를 순서대로 실행합니다. 같은 root branch의 여러 link 변경은 한 커밋으로 묶이며, root가 동시에 변경되면 자동 merge하지 않고 새 head의 link index가 생성된 후 재시도합니다.

자동 갱신은 같은 LoreHub에서 root 소유자가 접근 가능한 저장소 link에 적용됩니다. 자기 참조나 순환 link는 연속 자동 커밋을 방지하기 위해 건너뜁니다. 성공한 root push는 일반 push와 동일하게 자동 CI 및 다른 root 저장소의 link 전파를 다시 일으킬 수 있습니다.

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
| GET | `/api/v1/repositories/{name}/ci-config?branch={branch}` | branch 최신 revision의 `.lore-ci.toml` 원문 조회 |
| POST | `/api/v1/repositories/{name}/ci-config` | TOML 검증 후 `.lore-ci.toml` commit·push; CSRF와 기준 revision 필요 |
| POST | `/api/v1/repositories/{name}/ci-config/parse` | Visual 편집 전 TOML 검증·구조화; CSRF 필요 |
| POST | `/api/v1/repositories/{name}/ci-config/analyze` | `{content, changed_paths}` 읽기 전용 검증·오류 위치·경로 미리보기; 저장소 접근 권한과 CSRF 필요; 경로 최대 128개 |
| GET | `/api/v1/repositories/{name}/pipeline-branches` | 자동 CI 대상 branch 설정 조회 |
| POST | `/api/v1/repositories/{name}/pipeline-branches` | 자동 CI 대상 branch 설정 저장; CSRF 필요 |
| POST | `/api/v1/repositories` | repository 생성: 201; CSRF 필요 |
| POST | `/api/v1/repositories/{name}/delete` | repository 삭제: 204; CSRF 필요 |
| POST | `/api/v1/lore-token` | 로그인 사용자용 1시간 Lore JWT 발급; CSRF 필요 |
| POST | `/api/v1/pipelines` | 파이프라인 생성: 201 |
| GET | `/api/v1/pipelines` | 접근 가능한 저장소의 최근 100건. 기존 API 호환용 |
| GET | `/api/v1/pipeline-history?limit=100&before={pipeline_id}` | 최신순 실행 이력. 응답의 `next_before`를 다음 페이지의 `before`로 전달하며 최대 500건 |
| GET | `/api/v1/pipelines/{id}` | 파이프라인과 job 상태 |
| POST | `/api/v1/pipelines/{id}/cancel` | 저장소 접근 권한이 있는 실행 요청자 또는 관리자만 취소 요청 |
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

GitHub PR·push와 Lore push는 같은 검증 스크립트를 실행합니다. 로컬에서도 다음 명령으로 동일하게 검사할 수 있습니다.

```sh
RUSTUP_TOOLCHAIN=1.96 sh scripts/check.sh
```

필수 도구는 Rust 1.96(`rustfmt`, `clippy` 포함), Node.js 24 이상, Protobuf의 `protoc`, PostgreSQL 서버·백업 도구(`initdb`, `pg_ctl`, `pg_config`, `pg_dump`, `pg_restore`, `psql`)입니다. Linux/macOS의 일반 사용자 계정에서 실행합니다. `pg_config`가 있으면 서버 도구 경로를 자동으로 찾습니다. Rust 도구는 `rustup toolchain install 1.96 --profile minimal --component rustfmt --component clippy`로 준비할 수 있습니다.

스크립트는 다음 검사를 순서대로 실행하며 하나라도 실패하면 실패 종료 코드를 반환합니다.

1. Rust 포맷과 모든 target의 Clippy 검사(`-D warnings`).
2. `tests/*.test.mjs`의 JavaScript 회귀 테스트.
3. 실제 LoreHub 검증기로 루트 `.lore-ci.toml` 검증.
4. 임시 PostgreSQL에서 Rust 단위·통합·문서 테스트 전체 실행. `--include-ignored`로 DB가 필요한 테스트도 포함합니다.

`scripts/with-test-postgres.sh`는 실행마다 전용 임시 클러스터와 Unix socket을 만들고, 완료·실패·종료 신호 시 서버와 파일을 정리합니다. TCP 포트는 열지 않고 기존 `DATABASE_URL`과 PostgreSQL 연결 환경변수는 사용하지 않습니다. 병렬 실행도 서로 다른 클러스터를 사용합니다. 강제 종료(`SIGKILL`)나 호스트 장애처럼 정리 코드를 실행할 수 없는 경우에는 `/tmp/lorehub-ci.*`가 남을 수 있습니다.

특정 DB 테스트만 실행할 때도 같은 래퍼를 사용할 수 있습니다.

```sh
sh scripts/with-test-postgres.sh cargo test --locked --no-default-features --test pipeline_access -- --ignored
```

`Cargo.lock`을 버전 관리하고 `--locked`로 의존성을 고정합니다. CI에서는 배포용 Runner 설치 파일이 없어도 검사할 수 있도록 `--no-default-features`를 사용합니다. 설치 패키지 포함 release 빌드는 별도 배포 검증 대상입니다.

### 자동 실행 경로

- **GitHub:** `.github/workflows/ci.yml`이 모든 branch의 push와 PR, 수동 실행을 지원합니다. Ubuntu에서 Rust 1.96, Node.js 24, PostgreSQL·Protobuf 도구를 준비하고 공통 스크립트를 호출합니다. 같은 ref의 오래된 실행은 취소됩니다. 병합을 차단하는 필수 검사 설정은 GitHub branch protection에서 `Format, lint, and all tests`를 지정해야 합니다.
- **Lore:** 루트 `.lore-ci.toml`의 `verify` 파이프라인은 소스·웹·migration·proto·테스트·스크립트·예제·배포·GitHub 설정 폴더와 루트 설정·문서 파일 변경 시 Linux Runner에서 공통 스크립트를 실행합니다. Lore가 지원하는 정확한 경로 및 `directory/**` 규칙을 사용하므로 새로운 최상위 폴더나 파일을 추가하면 `changes`에도 포함하세요. Runner 계정에 위 도구를 미리 설치하고, LoreHub의 저장소 자동 CI branch 설정에 검증할 branch를 포함해야 합니다. 검증 파이프라인은 30분 제한이며 자동 배포는 하지 않습니다. 현재 배포용 Compose Runner 이미지에는 검증 도구 전체가 포함되어 있지 않으므로 준비된 Linux Runner가 필요합니다.

통합 테스트는 원자적 claim, lease fencing, 취소 경쟁, 로그인 세션·CSRF·입력 검증·로그 cursor, 저장소별 실행 조회 권한, 그룹·관리자 정책, stage 실행·실패 전파·정리, 타임아웃의 자식 프로세스 종료, 출력 제한, 로그 보존·일괄 삭제·롤백, 실제 백업 파일의 격리 복원과 손상 파일 거부를 검사합니다. 워커 테스트는 명령 인자를 검증하는 Lore CLI fixture를 사용하고 실제 shell과 PostgreSQL을 실행합니다. 실제 Google OAuth·Lore 서버 연결과 브라우저 전체 사용자 흐름 검증은 별도로 필요합니다.

## 저장소 중심 탐색

저장소 카드의 **실행 이력**, **파이프라인 그래프**, **CI 설정**, **저장소 링크**에서 해당 저장소로 바로 이동합니다. 화면 상단의 저장소 탐색 메뉴와 왼쪽 메뉴로 이 네 화면을 오갈 때 선택 범위가 유지됩니다. **전체 저장소 보기**로 범위를 해제할 수 있습니다. 선택은 URL의 `repository` query에 저장되어 새로고침과 브라우저 뒤로 가기에도 유지됩니다. 다른 전역 화면으로 이동하면 저장소 범위를 해제합니다.

실행 이력과 그래프 API는 전체 저장소 URL을 정확히 비교합니다. 실행 이력은 권한·저장소·branch·파이프라인 이름·상태·검색 조건을 모두 적용한 뒤 최신 100건을 가져옵니다. 따라서 최근 100건에 없는 과거 실행도 검색할 수 있습니다. **더 보기**로 과거 이력을 불러온 동안에는 자동 갱신이 목록을 초기화하지 않습니다. 새로고침 버튼이나 화면 재진입으로 최신 목록을 다시 불러옵니다.

Branch와 파이프라인 필터에는 **정확한 이름**을 입력합니다. 입력 후보는 현재 불러온 이력에서 제시하지만 후보에 없는 값도 검색할 수 있습니다. **실패** 상태 필터로 실패한 실행만 볼 수 있습니다. 상단 검색은 실행 ID·저장소 URL·표시 branch·revision·상태·파이프라인 이름·Runner OS·Sparse View 이름에서 대소문자를 구분하지 않는 부분 문자열을 찾습니다. `%`·`_`도 문자 그대로 처리하며 로그 본문은 검색하지 않습니다. 입력이 멈춘 뒤 250ms에 조회하고, 조건이 바뀌면 이전 페이지 cursor를 초기화합니다. 실행 이력 화면의 조건은 URL에 저장되어 새로고침·북마크로 복원됩니다. 다른 화면으로 이동하면 저장소 범위만 유지합니다. 목록 건수와 개요 통계는 현재 불러온 실행 기준이며 전체 검색 결과 수는 아닙니다.

`GET /api/v1/pipeline-history`와 `GET /api/v1/pipeline-graphs`는 선택적인 `repository_url` query를 지원하며 기존 저장소 접근 권한을 그대로 적용합니다. 이력 API는 `branch`, `pipeline_name`, `status`, `q`도 지원합니다. `status`는 `all`·`active`(대기/실행 중)·`finished`(성공/실패/취소)와 개별 상태를 받습니다. Branch·이름은 512자, 검색어는 256자까지이며 제어문자는 거부합니다. DB 쿼리는 5초 제한을 적용합니다. 다른 저장소 또는 접근 권한이 사라진 실행의 cursor는 거부하지만 cursor 실행의 상태 변경은 다음 페이지 조회를 막지 않습니다. 검색 조건이나 저장소가 바뀌기 전에 시작된 응답은 현재 목록을 덮어쓰지 않습니다. CI 편집 중 다른 범위로 이동할 때는 작성 중인 변경을 버릴지 확인합니다.

배포 시 저장소별 이력 조회 인덱스 migration `0027_repository_history.sql`을 적용합니다.

## 실행 상세 링크

실행 상세를 열면 주소에 `run`이 추가됩니다. 저장소·branch·검색 조건을 포함한 현재 주소로 새로고침하거나 북마크하면 해당 실행 상세를 다시 엽니다. 상세에서 **실행 링크 복사** 또는 **실행 링크 열기**를 사용하면 관리자 메뉴나 검색 조건을 제외한 공통 주소(`#pipelines?run=<실행 ID>`)를 얻습니다. 링크에는 인증 정보가 포함되지 않으며, 받는 사람에게 해당 저장소 접근 권한이 있어야 합니다.

목록에서 연 상세는 브라우저 뒤로 가기로 닫고 앞으로 가기로 다시 열 수 있습니다. 이때 이미 불러온 과거 이력과 필터는 유지합니다. 상세 안에서 연 관련 실행은 같은 상세 방문 기록을 교체하므로 뒤로 가기는 원래 목록으로 돌아갑니다. 공유 주소로 바로 진입하거나 새로고침한 상세를 닫으면 현재 페이지에 머무릅니다. 새로고침 후 목록은 첫 페이지부터 다시 불러오며 로그 읽기 위치는 저장하지 않습니다.

로그인이 필요한 링크는 로그인 버튼을 누를 때 같은 탭의 sessionStorage에 복귀 주소를 최대 10분간 저장하고, 로그인 완료 후 한 번만 복원합니다. 다른 주소로 진입하면 그 주소를 우선합니다. 브라우저가 탭 저장소를 차단하는 경우 로그인 후 실행 링크를 다시 여세요. 실제 Google OAuth 연결은 별도 배포 환경 검증 대상입니다.

## 실행 상세의 대용량 로그

실행 상세는 상태·그래프·작업 목록을 먼저 표시한 뒤 전체 실행 로그의 첫 500건을 가져옵니다. 로그가 한 페이지를 채우면 **로그 500건 더 보기**로 다음 부분을 읽습니다. 첫 화면에서 전체 로그를 자동으로 내려받지 않습니다. 마지막 페이지까지 읽은 뒤에는 **새 출력 따라가기**가 켜져 있을 때 주기적으로 새 출력을 확인합니다.

로그를 위로 스크롤하거나 **새 출력 따라가기**를 끄면 전체 실행 로그의 자동 조회와 스크롤을 멈춥니다. 실행 상태·작업 정보는 계속 갱신되며 그래프에서 선택한 작업의 로그는 별도 영역입니다. **처음부터 보기**는 첫 페이지를 다시 읽습니다. 화면에는 최대 2,000개 로그 레코드와 본문 512,000 UTF-16 코드 단위만 보관합니다. 상한을 넘으면 먼저 읽은 부분을 화면에서 제거하고 안내하며, DB 로그는 삭제하지 않습니다.

로그 조회 실패 시 같은 cursor에서 재시도합니다. 상세를 닫거나 다른 실행으로 이동한 뒤의 응답은 반영하지 않습니다. 보존 정책에 따른 로그 정리를 감지하면 캐시를 비우고 남은 로그부터 다시 읽습니다. 상세 조회 실패 또는 로그 접근 권한 상실 시 이전 실행 내용을 지우고 재시도 버튼을 표시합니다.

## 운영 상태 진단

관리자 메뉴의 **운영 상태**(`#operations`)에서 대기 작업, Runner 가용성, 저장소 확인 결과와 PostgreSQL 사용량을 함께 확인합니다. `GET /api/v1/operations`는 로그인한 관리자만 사용할 수 있고, 계정 강등은 기존 세션에도 즉시 적용됩니다. 실행 요약과 목록에는 기존 저장소 접근 범위에 포함되는 실행만 반영합니다. Runner와 물리 DB 용량은 전체 서비스 기준입니다.

- **대기 작업:** 대기·실행 중 건수, 가장 긴 대기 시간, lease가 만료된 실행 수를 표시합니다. 가장 오래 대기한 100건을 취소 처리, 선행 실행 결과 대기, 대상 OS Runner 없음, 대상 Runner 모두 실행 중, 수행 요청 가능한 상태로 구분합니다. 선행 실행은 실제 claim과 같이 동일 저장소·branch·revision·이름의 가장 최근 실행을 확인하며, 없는 선행 실행을 임의로 대기 원인으로 만들지 않습니다. 실행 이름을 누르면 상세로 이동합니다.
- **Runner:** OS별 연결·유휴·배정 중지·연결 끊김·종료 수입니다. 연결 기준은 기존 heartbeat 기준인 15초이며, 실행 중인 작업이 없고 배정이 허용된 Runner만 유휴로 집계합니다. 배정 중지 수에는 오프라인 Runner도 포함됩니다. 가용성은 관측 시점의 정보이므로 수행 시작을 보장하지 않습니다.
- **저장소:** watcher가 CI 확인과 링크 목록 확인을 마친 시각을 저장합니다. revision 변경이 없어도 성공 관측을 갱신합니다. 정상·실패·2분 초과 확인 지연·아직 관측되지 않음을 구분하며, 최근 성공과 연속 실패 수를 함께 보여줍니다. 확인 지연은 실제 revision 차이나 동기화 손실을 의미하지 않습니다. 긴 clone·검사나 coordinator 중단도 지연으로 나타날 수 있습니다. 오류·지연을 우선하여 최대 100개 저장소를 표시합니다.
- **링크 갱신 오류:** 현재 존재하는 링크에 저장된 최근 오류 개수입니다. watcher 확인이 성공해도 링크 갱신 오류는 별도로 남을 수 있습니다. 저장소 링크 화면에서 해당 작업과 정책을 확인하세요.
- **용량:** PostgreSQL 전체 DB, 실행·job 테이블, 로그 테이블의 물리 크기입니다. 인덱스·TOAST를 포함하며, 전체 DB 값에 다른 항목이 포함되어 있으므로 합산하지 않습니다. 디스크 여유 공간, Lore local 데이터와 S3 용량은 측정하지 않습니다.

화면이 열려 있을 때 30초마다 갱신하고 마지막 관측 시각을 표시합니다. 갱신 실패 시 이전 데이터를 정상 상태처럼 남겨두지 않으며 다시 시도할 수 있습니다. 자동 알림·재시도 실행·정리 기능은 포함하지 않습니다. DB 조회는 읽기 전용 일관된 snapshot과 쿼리별 5초 제한을 사용합니다.

배포 시 migration `0026_repository_watch_health.sql`이 필요합니다. watcher 오류는 `watch_failed`, `link_index_failed`, `backend_unavailable` 등 고정된 종류만 저장하며, 명령 인자·토큰·Lore 원문 출력은 진단 API에 포함하지 않습니다. 상세 원인은 해당 저장소의 coordinator 로그에서 확인합니다. 새 설치에서 관측 기록이 없으면 첫 확인이 끝날 때까지 정상으로 표시하지 않습니다.

## 로그 보존과 PostgreSQL 복구

### 완료된 실행의 로그 정리

`lorehub prune-logs`는 DB에 직접 접근할 수 있는 운영자용 명령입니다. 기본 동작은 **삭제 없는 미리보기**이며, HTTP API에는 삭제 기능을 노출하지 않습니다. 먼저 새 바이너리의 `lorehub migrate` 또는 coordinator 시작으로 migration `0025_log_retention.sql`을 적용합니다. 정리 명령 자체는 migration을 실행하지 않습니다.

```sh
# DATABASE_URL은 대상 PostgreSQL로 설정되어 있어야 합니다.
# 30일은 예시이며 운영 정책에 맞춰 선택합니다.
lorehub prune-logs --keep-days 30

# 미리보기 확인과 백업 후, 최대 1,000행만 삭제합니다.
lorehub prune-logs --keep-days 30 --batch-size 1000 --apply
```

`--keep-days`(1~36,500일)는 `LOREHUB_LOG_RETENTION_DAYS` 환경변수로도 지정할 수 있습니다. 환경변수를 설정하는 것만으로 자동 정리가 시작되지는 않습니다. `--apply`를 명시한 호출만 삭제하며 기본 1,000행, 최대 10,000행을 한 트랜잭션으로 처리합니다. 대량 정리는 결과를 확인하며 같은 명령을 반복합니다. 내장 스케줄러는 없습니다.

JSON 결과의 `cutoff`는 DB 시각 기준 보존 경계, `eligible`은 호출 시점의 전체 삭제 대상 실행 수·로그 행 수·본문 바이트, `deleted`는 이번 호출에서 실제 삭제한 양입니다. 미리보기와 실제 실행 사이에는 대상이 달라질 수 있습니다. `content_bytes`는 UTF-8 본문 크기이며 디스크 회수량이 아닙니다. 삭제된 공간은 PostgreSQL vacuum 이후 재사용될 수 있으며 파일 크기가 즉시 줄어들지는 않습니다.

- `succeeded`·`failed`·`canceled` 상태이면서 `finished_at`이 경계보다 이전인 실행만 대상입니다. 로그 작성 시각이 아니라 **실행 종료 시각**부터 보존 기간을 셉니다. 진행 중·대기 중·종료 시각이 없는 실행은 보존합니다.
- 실행, job, 그래프, revision 번호, 의존 관계와 push 중복 방지 기록은 유지합니다. 실행 상세에는 일부 또는 전체 로그가 정리되었다는 안내가 표시됩니다. 삭제한 로그를 다시 보려면 백업이 필요합니다.
- 동시 정리 호출은 잠금으로 차단하고, 다른 트랜잭션이 잠근 로그 행은 건너뜁니다. 따라서 삭제 건수가 0이어도 `eligible`이 남아 있으면 다음에 다시 확인합니다. statement 30초·lock 5초 제한을 적용하며 실패한 배치는 전체 롤백됩니다. 미리보기 집계도 30초 제한을 적용합니다.

### 백업 생성과 격리 복원 점검

PostgreSQL 서버와 호환되는 `pg_dump`·`pg_restore`·`psql` 및 `initdb`·`pg_ctl`이 필요합니다. 백업 URL은 libpq가 지원하는 PostgreSQL URL을 사용합니다. 복원 점검은 Linux/macOS의 일반 사용자로 실행합니다.

```sh
mkdir -p /secure/backups/lorehub
chmod 700 /secure/backups/lorehub
# 기존 DATABASE_URL을 읽어 고유한 디렉터리에 custom-format 백업을 만듭니다.
sh scripts/backup-postgres.sh /secure/backups/lorehub

# 위 명령이 출력한 파일 경로를 전달합니다.
sh scripts/verify-postgres-backup.sh /secure/backups/lorehub/LOREHUB_BACKUP_DIRECTORY/lorehub.dump
```

백업 스크립트는 기존 파일을 덮어쓰지 않으며 디렉터리와 파일을 소유자만 접근 가능하게 생성합니다. `pg_dump` 또는 archive 목록 확인이 실패하면 불완전한 백업을 정리합니다. 파일을 만든 것만으로 복구 가능성을 판단하지 말고 복원 점검까지 실행하세요. URL을 도구 인자로 전달하므로 비밀번호 대신 `.pgpass`를 사용할 수 있습니다.

복원 점검은 기존 `DATABASE_URL`·PostgreSQL 연결 환경변수를 무시하고, 전용 임시 클러스터의 Unix socket에만 접속합니다. 신뢰할 수 있는 자체 백업만 사용합니다. `pg_restore --single-transaction --exit-on-error`로 실제 복원하고 migration 성공 여부, 실행·job·로그·저장소·사용자 행 수와 로그 본문 크기를 확인한 뒤 클러스터를 정리합니다. 운영 DB에 복원하는 기능은 없습니다. 일반 종료와 오류·종료 신호는 정리하지만 `SIGKILL`이나 호스트 장애 시 임시 디렉터리가 남을 수 있습니다.

이 점검은 **PostgreSQL 백업의 복원 가능성**을 확인합니다. 복원에는 소유권·권한을 적용하지 않으므로 실제 복구에서는 DB 계정과 권한을 별도로 준비해야 합니다. 전체 서비스 복구에는 PostgreSQL 외에 Lore의 `dynamodb_data`와 대응 S3 payload, local backend 데이터, JWT 키·환경 설정이 필요합니다. 복구 지점을 맞추려면 새 CI 제출과 저장소 변경을 멈추고 실행 중 작업을 정리한 상태에서 이들을 함께 보관합니다. S3만으로는 저장소를 복구할 수 없습니다.

실제 장애 복구는 별도 환경에서 백업을 복원하고 동일 버전 바이너리로 확인한 다음 전환합니다. 검증 중에는 coordinator와 Runner의 자동 실행을 시작하지 않습니다. 복원 후 실행 중으로 남은 작업은 원래 프로세스와 이어지지 않으며 lease 만료 후 실패 처리됩니다. 로그인·저장소 조회·실행 이력·로그를 확인하고, 외부 부수 효과를 확인한 작업만 수동으로 재실행하세요. 자동화된 CI 복원 테스트는 작은 fixture 기준이므로 운영 크기의 백업으로 복원 시간과 허용 가능한 데이터 손실 범위도 별도 측정해야 합니다.

## 서비스 운영 및 다음 단계

`deploy/`의 systemd unit 예제를 사용할 수 있습니다. API coordinator 바이너리는 `/usr/local/bin/lorehub`에 설치하고, Runner 바이너리는 아래 설치 스크립트로 전용 state 디렉터리에 배치합니다. `lorehub` OS 계정과 `/etc/lorehub/environment` 환경 파일을 준비하고 워커 unit의 PATH를 설치된 빌드 도구 위치에 맞게 조정하세요. 환경 파일은 해당 서비스 관리자만 읽을 수 있게 설정합니다.

Coordinator를 재시작할 때는 배포 환경에 맞는 서비스 관리자를 자동 감지하는 스크립트를 사용할 수 있습니다.
스크립트는 재시작 전에 `cargo build --locked --release`를 실행하며, Linux systemd 환경에서는 새 coordinator 바이너리를 `/usr/local/bin/lorehub`에 설치합니다.

```bash
sudo sh deploy/restart-lorehub.sh       # API coordinator
sudo sh deploy/restart-lorehub.sh all   # API + 실행 중인 systemd worker
```

PostgreSQL, 두 Lore storage backend, TLS proxy와 coordinator를 빌드하고 시작하려면 전체 재시작 스크립트를 사용합니다. Compose의 Linux runner는 이 스크립트에서 시작하거나 재시작하지 않습니다. 인증서 최초 발급용 일회성 `certbot` 서비스는 포함하지 않고 자동 갱신 서비스만 시작합니다.

```bash
sh deploy/restart-all.sh
```

Compose runner를 별도로 갱신해야 할 때만 다음 명령을 사용합니다.

```bash
docker compose --profile runner up -d --build runner
```

Linux에서는 `lorehub-api.service`와 실행 중인 `lorehub-worker@*.service`를 재시작하고, macOS에서는 `co.kr.zenogrid.lorehub.coordinator` LaunchDaemon을 kickstart합니다. LaunchDaemon을 설치하지 않고 `target/release/lorehub serve`를 직접 실행한 macOS 개발 환경도 기존 프로세스를 찾아 `deploy/macos/run-coordinator.sh`로 재시작합니다. macOS Runner LaunchDaemon은 별도로 관리합니다.

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

### Runner 통신 장애 복구

Runner 등록·작업 수신·완료 보고는 연결 실패·타임아웃 및 HTTP `408/429/500/502/503/504`에 한해 최대 4회 시도합니다. 각 요청의 제한 시간은 5초이며, 재시도 사이에는 250ms부터 증가하는 대기 시간에 작은 무작위 지연을 더합니다. 인증·권한 오류나 잘못된 응답 데이터는 자동 재시도하지 않습니다.

일반 실행 모드는 등록이나 작업 수신의 재시도를 모두 소진해도 종료하지 않고, 2초부터 최대 30초까지 대기 간격을 늘려 재접속합니다. 종료 신호는 이 대기와 진행 중인 등록·수신 요청을 중단합니다. `worker --once`는 제한된 요청 재시도 후 실패를 반환합니다. 완료 보고를 모두 실패한 일반 Runner는 경고를 남기고 작업 수신을 계속하며, 같은 Runner의 기존 실행 lease가 살아 있는 동안에는 새 작업을 배정하지 않습니다.

작업 수신은 `/api/v1/runner/claim/{request_id}`를 사용합니다. 네트워크 오류 후에도 같은 요청 ID를 유지하며, 서버는 배정 결과를 실행 기록에 함께 저장합니다. 같은 요청의 동시 재전송에는 같은 실행을 반환하고, 이미 완료·취소·만료된 배정 요청으로는 다른 작업을 받거나 기존 작업을 재실행하지 않습니다. 실제 작업 스크립트는 재시도 대상이 아닙니다. 완료 재보고도 기존 종료 결과를 덮어쓰지 않습니다. 로그 추가·job 생성은 아직 멱등 요청이 아니므로 자동 재전송하지 않습니다.

기존 5초 heartbeat, 연속 3회 heartbeat 실패 시 실행 취소, 30초 lease 만료 처리는 유지됩니다. 장시간 단절이나 Runner 프로세스 재시작 후 실행·완료 보고를 복구하는 영속 큐는 제공하지 않습니다.

**배포 순서:** `0028_runner_claim_requests.sql` migration이 적용되는 Coordinator를 먼저 배포한 뒤 Runner를 업데이트합니다. 기존 Runner의 `/api/v1/runner/claim` 경로도 유지합니다. 새 Runner는 구형 Coordinator가 요청 ID를 무시하고 중복 배정하는 일을 막기 위해 기존 경로로 자동 전환하지 않으며, 새 경로가 없는 서버에서는 `404`로 종료합니다.

### Runner 유지보수 모드

관리자는 **Runners → 배정 중지**로 Runner의 신규 작업 수신을 중단할 수 있습니다. 이미 배정된 파이프라인은 취소하지 않고 정상적으로 마무리하며, 화면은 **작업 마무리 중 → 유지보수 모드**로 전환됩니다. 연결 상태인 online/offline은 별도로 표시합니다. 현재 실행을 조회할 저장소 권한이 없으면 실행 ID와 링크를 숨기고 실행 여부만 표시합니다.

유지보수 모드가 된 것을 확인한 뒤 OS 서비스 관리자로 Runner를 중지하고 점검하세요. 이 기능 자체가 Runner 프로세스나 셀프 업데이트를 중지하지는 않습니다. 점검 후 **배정 재개**를 누르면 새 작업을 받을 수 있습니다. 설정은 Coordinator DB에 저장되어 heartbeat·재접속·재등록 뒤에도 유지되며, 등록을 삭제하고 다시 생성하면 기본값(배정 허용)으로 돌아갑니다.

`POST /api/v1/runners/{id}/drain`은 관리자 세션과 CSRF 토큰, JSON `{"draining": true}`(중지) 또는 `{"draining": false}`(재개)를 받습니다. 같은 값을 반복해도 결과는 같습니다. 배정 트랜잭션과 전환 요청은 Runner 행 잠금으로 직렬화합니다. 중지 응답을 받은 이후에는 새로운 배정을 만들지 않으며, 중지 직전에 이미 배정한 작업의 요청 ID 재전송은 기존 실행을 반환해 마무리할 수 있게 합니다. 기존 Runner의 claim 경로에도 중지가 적용됩니다.

운영 상태의 유휴 Runner 수는 배정 중지된 Runner를 제외합니다. 대상 OS의 연결된 Runner가 모두 중지 상태라면 대기 원인에 이를 표시합니다. `0029_runner_drain.sql` migration과 웹 화면을 포함한 Coordinator 배포로 적용되며, 이 기능을 위한 Runner 바이너리 교체는 필요하지 않습니다.

### Runner 상태 진단

Runners 목록에서 **Runner 이름**을 누르면 최근 등록, 마지막 연결 보고, 마지막 작업 요청, 종료 보고, 관측 시각을 확인합니다. Docker 정보는 등록 시 CLI 설치 확인 결과이며 daemon의 현재 실행 여부를 뜻하지 않습니다. 진단은 로그인한 사용자가 볼 수 있고, 작업 배정 중지·재개는 기존처럼 관리자만 가능합니다.

- **종료 보고됨 / 연결 확인 지연:** Runner의 명시적인 종료 보고와 15초 넘게 heartbeat가 없는 상태를 구분합니다. 연결 끊김만으로 프로세스 종료나 네트워크 장애 원인을 단정하지 않습니다.
- **작업 요청 지연:** heartbeat는 유지되지만 유휴 Runner의 작업 요청이 60초 넘게 없는 상태입니다. 등록 후 아직 요청이 없으면 등록 시각을 기준으로 판단합니다. 자동 업데이트 진행, Runner 로그와 Coordinator 연결을 확인하세요.
- **작업 실행 중 / 작업 마무리 중 / 유지보수 모드:** 긴 작업 때문에 작업 요청이 없는 것을 수신 장애로 표시하지 않습니다. 유지보수 설정도 별도로 반영합니다.

작업을 배정하지 않은 빈 응답, 유지보수 중 요청, 동일 요청 ID의 재전송도 마지막 작업 요청에 반영합니다. 등록 시에는 이전 요청 시각을 초기화합니다. 화면의 시각과 진단은 Coordinator 관측 기준이며, 조회 실패 시 마지막 정보라는 안내를 표시합니다. 원격 호스트의 CPU·메모리·디스크 또는 프로세스 로그는 수집하지 않습니다.

`0030_runner_poll_diagnostics.sql`을 포함한 Coordinator 배포가 필요하며 기존 Runner의 claim API에도 적용됩니다. 이번 변경만으로 Runner 바이너리를 다시 설치할 필요는 없습니다.

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

Runner는 JWT로 인증된 coordinator HTTP API를 통해 등록, claim, heartbeat, 상태 및 로그를 처리하며 PostgreSQL에 직접 연결하지 않습니다. 파일 본문 미리보기, OS 외의 runner tag, container executor, job DAG/병렬 실행, artifact/cache 업로드, secret 관리 및 실행 메타데이터 보존 기간 정리는 아직 구현하지 않았습니다. 이 기능들은 각각 executor/트리거/스토리지 계층으로 확장할 수 있습니다.

### 계정, 계정 그룹, sparse workspace view

사이드바에서 **계정 관리** (`#accounts`), **계정 그룹 관리** (`#account-groups`),
**Workspace view 설정** (`#workspace-views`) 화면을 사용할 수 있습니다. 모바일에서는 페이지
선택 메뉴로 이동합니다. 기존 언어 설정(한국어·영어·중국어)을 따릅니다. 현재 이 관리 화면과
아래 관리 API는 모두 관리자 전용이며 일반 사용자·그룹 구성원은 `403`을 받습니다.

- 계정은 Google Workspace 최초 로그인 시 등록됩니다. 관리자는 계정 목록을 조회하고
  자신의 표시 이름을 수정할 수 있습니다. 표시 이름은 Google 재로그인 후에도 유지됩니다.
- 그룹 생성자는 이름·설명과 기존 계정의 구성원 목록을 관리합니다. 생성자는 항상 구성원으로
  포함되며, 관리자는 전체 그룹을 조회할 수 있지만 다른 소유자의 그룹은 수정할 수 없습니다.
- 관리자는 모든 리포지토리에서 본인 view 프리셋을 만들고 본인 그룹에 연결할 수 있습니다.
  다른 소유자의 프리셋은 조회만 가능합니다. 그룹의 view 프리셋 연결과 리포지토리 접근 권한
  부여는 별도이며, 접근 권한을 명시적으로 부여받은 그룹 구성원은 저장소와 실행 정보를
  조회할 수 있어도 이 관리 API에는 접근할 수 없습니다.
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
| `GET /api/v1/account-groups` | 전체 그룹 목록 |
| `POST /api/v1/account-groups` | 그룹 생성 (`name`, `description`, `member_ids`) |
| `POST /api/v1/account-groups/{id}` | 그룹 수정 |
| `DELETE /api/v1/account-groups/{id}` | 그룹 삭제 |
| `GET /api/v1/workspace-repositories` | 프리셋을 만들 수 있는 전체 리포지토리 목록 |
| `GET /api/v1/account-groups/{id}/views` | 그룹의 저장된 view 목록 |
| `POST /api/v1/account-groups/{id}/views/{resource_id}` | 기존 프리셋 연결 (`view_id`) |
| `DELETE /api/v1/account-groups/{id}/views/{resource_id}` | 프리셋 삭제 |

통합 테스트는 폐기 가능한 PostgreSQL 인스턴스에서 실행합니다.

```sh
sh scripts/with-test-postgres.sh cargo test --locked --no-default-features --test management -- --ignored
```
