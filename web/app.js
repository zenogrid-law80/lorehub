"use strict";

const SUPPORTED_LOCALES = ["en", "ko", "zh-CN"];
const SUPPORTED_THEMES = ["system", "light", "dark"];
const THEME_MEDIA_QUERY = window.matchMedia("(prefers-color-scheme: dark)");
const DEFAULT_CI_CONFIG = `stages = ["build"]

[[jobs]]
name = "build"
stage = "build"
script = ["echo Configure your CI job"]
`;
const DEFAULT_CI_MODEL = {
  stages: ["build"],
  jobs: [{ name: "build", stage: "build", needs: [], script: ["echo Configure your CI job"], timeout_seconds: 3600 }],
  pipelines: [],
};
const I18N = {
  en: {
    "Create .lore-ci.toml": "Create .lore-ci.toml",
    "Use template": "Use template",
    "This repository has no CI configuration. Start with a build template, edit the commands, then save.": "This repository has no CI configuration. Start with a build template, edit the commands, then save.",
    "CI settings are unavailable for Lore link source repositories.": "CI settings are unavailable for Lore link source repositories.",
    "Select a repository and open CI settings to create a pipeline run.": "Select a repository and open CI settings to create a pipeline run.",
    "Selected branch is no longer available. Refresh CI settings.": "Selected branch is no longer available. Refresh CI settings.",
    "워크스페이스를 불러오는 중…": "Loading workspace…", "LoreHub 홈": "LoreHub home", "코드에서 배포까지,": "From code to deployment,", "하나의 흐름으로.": "in one workflow.",
    "Lore revision을 안전하게 실행하고 파이프라인의 모든 단계를 한곳에서 확인하세요.": "Run Lore revisions safely and see every pipeline stage in one place.", "LoreHub에 로그인": "Sign in to LoreHub", "조직 계정으로 로그인해 프로젝트와 CI 파이프라인을 관리합니다.": "Sign in with your organization account to manage projects and CI pipelines.", "Google 계정으로 계속": "Continue with Google", "@zenogrid.co.kr 계정만 사용할 수 있습니다": "Only @zenogrid.co.kr accounts are supported", "Lore VCS 기반 개발 플랫폼": "Development platform built on Lore VCS",
    "주 메뉴": "Main menu", "파이프라인 검색": "Search pipelines", "새로고침": "Refresh", "로그아웃": "Sign out", "현재 워크스페이스의 파이프라인 상태입니다.": "Pipeline status for the current workspace.", "파이프라인 요약": "Pipeline summary", "최근 100건": "Latest 100", "성공률 —": "Success rate —", "최신 Lore revision 실행 내역": "Latest Lore revision runs", "상태 필터": "Status filter", "파이프라인이 없습니다": "No pipelines", "Lore repository와 revision을 지정해 첫 실행을 시작하세요.": "Choose a Lore repository and revision to start the first run.", "업데이트 대기 중": "Waiting for update",
    "폴더 변경이 어떤 파이프라인과 Runner, stage를 실행하는지 확인합니다.": "See which pipeline, Runner, and stages each folder change triggers.", "Pipeline graph 요약": "Pipeline graph summary", "최근 pipeline별 경로": "Routes for the current revision", "고유한 changes 규칙": "Unique change rules", "대상 운영체제": "Target operating systems", "각 pipeline의 가장 최근 실행 snapshot을 표시합니다": "Shows the latest run snapshot for each pipeline", "표시할 routing graph가 없습니다": "No routing graphs", "changes와 runner_os가 있는 pipeline이 실행되면 여기에 표시됩니다.": "Pipelines with changes and runner_os appear here.", "그래프를 선택하면 실행 상세를 엽니다.": "Select a graph to open run details.",
    "현재 Lore 서버의 repository를 생성하고 연결 주소를 관리합니다.": "Create repositories on the current Lore server and manage connection URLs.", "서버에서 조회한 Lore repository": "Lore repositories loaded from the server", "Repository가 없습니다": "No repositories", "현재 Lore 서버에 첫 repository를 만드세요.": "Create the first repository on the current Lore server.", "Google Workspace 로그인 사용자만 관리할 수 있습니다.": "Only signed-in Google Workspace users can manage repositories.",
    "등록된 runner의 운영체제, 연결 상태와 현재 작업을 확인합니다.": "Check each registered Runner's operating system, connection, and current job.", "Runner 다운로드": "Download Runner", "LoreHub runner와 Lore CLI, 설치 스크립트가 포함된 x86_64 패키지입니다.": "x86_64 packages containing the LoreHub Runner, Lore CLI, and install scripts.", "Runner 요약": "Runner summary", "등록된 전체 runner": "All registered Runners", "15초 내 heartbeat": "Heartbeat within 15 seconds", "종료 또는 heartbeat 만료": "Stopped or heartbeat expired", "5초 간격으로 상태가 갱신됩니다": "Status refreshes every 5 seconds", "등록된 runner가 없습니다": "No registered Runners", "Linux 또는 Windows에서 lorehub worker를 실행하면 자동으로 표시됩니다.": "Run a lorehub worker on Linux or Windows and it will appear automatically.", "Offline runner도 이력으로 유지됩니다.": "Offline Runners remain in history.",
    "닫기": "Close", "실행할 Lore 저장소와 변경되지 않는 64자리 revision hash를 입력하세요.": "Enter the Lore repository and immutable 64-character revision hash to run.", "lores:// 주소": "lores:// URL", "branch나 head 대신 전체 revision hash": "Full revision hash instead of a branch or head", "현재 서버에 새로운 Lore repository를 생성합니다.": "Create a new Lore repository on the current server.", "영문자·숫자로 시작하고 끝나며 점, 대시, 밑줄 사용 가능": "Start and end with a letter or number; dots, dashes, and underscores are allowed.", "repository와 서버에 저장된 데이터를 삭제합니다. 이 작업은 되돌릴 수 없습니다.": "Delete the repository and its server data. This action cannot be undone.", "확인을 위해 repository 이름 입력": "Enter the repository name to confirm", "Google 로그인 계정으로 발급된 토큰입니다. 1시간 동안 유효하며 다른 사람과 공유하지 마세요.": "This token was issued for your Google account. It is valid for one hour; do not share it.", "로그를 불러오는 중…": "Loading logs…",
    "Total pipelines": "Total pipelines", "Succeeded": "Succeeded", "Active": "Active", "Needs attention": "Needs attention", "Recent pipelines": "Recent pipelines", "Finished": "Finished", "Created": "Created", "Duration": "Duration", "New pipeline": "New pipeline", "Pipeline graphs": "Pipeline graphs", "Routes": "Routes", "Folder rules": "Folder rules", "Runner targets": "Runner targets", "Folder routing map": "Folder routing map", "New repository": "New repository", "Current Lore server": "Current Lore server", "Repository list": "Repository list", "Total runners": "Total runners", "Registered runners": "Registered runners", "Operating system": "Operating system", "Architecture": "Architecture", "Version": "Version", "Last seen": "Last seen", "Current job": "Current job", "optional": "optional",
    "dynamic.errorUser": "Could not load sign-in information.", "dynamic.sessionExpired": "Your session has expired.", "dynamic.requestFailed": "Request failed ({status}).", "dynamic.updated": "Updated now · {time}", "dynamic.refreshPipelines": "Pipelines refreshed.", "dynamic.refreshRepositories": "Repositories refreshed.", "dynamic.refreshRunners": "Runner status refreshed.", "dynamic.refreshGraphs": "Pipeline graphs refreshed.", "dynamic.beforeRun": "{branch} · no runs yet", "dynamic.viewRun": "View run", "dynamic.noRuns": "No runs yet", "dynamic.idle": "Idle", "dynamic.copyUrl": "Copy URL", "dynamic.runPipeline": "Run pipeline", "dynamic.delete": "Delete", "dynamic.urlCopied": "Repository URL copied.", "dynamic.copyFailed": "Could not copy to the clipboard.", "dynamic.repositoryCreated": "Repository created.", "dynamic.repositoryDeleted": "Repository deleted.", "dynamic.nameMismatch": "The repository name must match.", "dynamic.tokenCopied": "Lore access token copied.", "dynamic.pipelineQueued": "New pipeline added to the queue.", "dynamic.starting": "Starting…", "dynamic.loading": "Loading…", "dynamic.loadingLogs": "Loading logs…", "dynamic.workerPreparing": "The worker is preparing the job configuration.", "dynamic.noLogs": "No log output yet.", "dynamic.logsPruned": "Older logs were removed under the retention policy. Any remaining output is shown below.\n", "dynamic.cancelRequested": "Pipeline cancellation requested.",
    "dynamic.successRate": "Success rate {rate}", "dynamic.successRateEmpty": "Success rate —", "dynamic.countPipelines": "{count} pipelines", "dynamic.countFilteredPipelines": "{shown} / {total} pipelines", "dynamic.countRoutes": "{count} routes", "dynamic.countFilteredRoutes": "{shown} / {total} routes", "dynamic.countRepositories": "{count} repositories", "dynamic.countFilteredRepositories": "{shown} / {total} repositories", "dynamic.countRunners": "{count} Runners", "dynamic.countFilteredRunners": "{shown} / {total} Runners", "dynamic.pipelineTitle": "Pipeline #{id}", "dynamic.jobs": "{count} jobs", "dynamic.matchingPaths": "{count} matching path(s)", "dynamic.pathRule": "path rule", "dynamic.changedFolder": "Changed folder", "dynamic.runsIn": "runs in {directory}", "dynamic.repositoryRoot": "repository root", "dynamic.assigned": "{os} · assigned", "dynamic.target": "{os} target", "dynamic.waiting": "waiting for {os}", "dynamic.stage": "Stage · {status}", "dynamic.graphAria": "A change to {patterns} runs pipeline {pipeline} on a {os} Runner through stages {stages}.", "dynamic.greetingMorning": "Good morning, {name}.", "dynamic.greetingAfternoon": "Good afternoon, {name}.", "dynamic.greetingEvening": "Good evening, {name}.", "dynamic.detail": "View details",
    "status.configured": "Configured", "status.queued": "Queued", "status.running": "Running", "status.succeeded": "Passed", "status.failed": "Failed", "status.canceled": "Canceled", "status.skipped": "Skipped", "status.online": "Online", "status.offline": "Offline"
  },
  ko: {
    "Create .lore-ci.toml": ".lore-ci.toml 생성",
    "Use template": "템플릿으로 시작",
    "This repository has no CI configuration. Start with a build template, edit the commands, then save.": "CI 설정 파일이 없습니다. 빌드 템플릿으로 시작해 실행 명령을 수정한 뒤 저장하세요.",
    "CI settings are unavailable for Lore link source repositories.": "다른 저장소에서 Lore 링크 원본(Source Repository)으로 참조하는 저장소는 CI settings를 사용할 수 없습니다.",
    "Select a repository and open CI settings to create a pipeline run.": "저장소를 선택한 뒤 CI settings에서 새 실행을 생성하세요.",
    "Selected branch is no longer available. Refresh CI settings.": "선택한 브랜치가 더 이상 없습니다. CI settings를 새로고침하세요.",
    "Language": "언어", "LORE NATIVE DEV PLATFORM": "LORE 네이티브 개발 플랫폼", "ZENOGRID WORKSPACE": "ZENOGRID 워크스페이스", "Overview": "개요", "Pipelines": "파이프라인", "Graphs": "그래프", "Repositories": "저장소", "Runners": "Runner", "Workspace": "워크스페이스", "Internal": "내부", "Coordinator online": "Coordinator 온라인", "Search pipelines…": "파이프라인 검색…", "Total pipelines": "전체 파이프라인", "Succeeded": "성공", "Active": "활성", "Queued + running": "대기 + 실행 중", "Needs attention": "확인 필요", "Failed pipelines": "실패한 파이프라인", "Recent pipelines": "최근 파이프라인", "All": "전체", "Finished": "완료", "Status": "상태", "Pipeline": "파이프라인", "Revision": "Revision", "Created": "생성 시각", "Duration": "소요 시간", "Actions": "작업", "New pipeline": "새 파이프라인", "CI routing": "CI 라우팅", "Pipeline graphs": "파이프라인 그래프", "Routes": "경로", "Folder rules": "폴더 규칙", "Runner targets": "Runner 대상", "Folder routing map": "폴더 라우팅 맵", "Lore server": "Lore 서버", "New repository": "새 저장소", "Current Lore server": "현재 Lore 서버", "CLI access token": "CLI 액세스 토큰", "Online": "온라인", "Repository list": "저장소 목록", "CI infrastructure": "CI 인프라", "Runner 다운로드": "Runner 다운로드", "Total runners": "전체 Runner", "Offline": "오프라인", "Registered runners": "등록된 Runner", "Runner": "Runner", "Operating system": "운영체제", "Architecture": "아키텍처", "Version": "버전", "Last seen": "마지막 연결", "Current job": "현재 작업", "RUN CI": "CI 실행", "Repository URL": "저장소 URL", "Cancel": "취소", "Run pipeline": "파이프라인 실행", "LORE SERVER": "LORE 서버", "Repository name": "저장소 이름", "Storage Backend": "Storage Backend", "DynamoDB + S3": "DynamoDB + S3", "Local File": "Local File", "Description": "설명", "optional": "선택", "Repository purpose": "저장소 용도", "Create repository": "저장소 생성", "DANGER ZONE": "위험 구역", "Delete repository": "저장소 삭제", "CLI AUTHENTICATION": "CLI 인증", "Lore access token": "Lore 액세스 토큰", "Access token": "액세스 토큰", "Copy token": "토큰 복사", "Close": "닫기", "Execution graph": "실행 그래프", "folder routing": "폴더 라우팅", "Jobs": "작업", "Job log": "작업 로그", "live": "실시간", "Cancel pipeline": "파이프라인 취소",
    "dynamic.errorUser": "로그인 정보를 불러오지 못했습니다.", "dynamic.sessionExpired": "세션이 만료되었습니다.", "dynamic.requestFailed": "요청에 실패했습니다 ({status}).", "dynamic.updated": "방금 업데이트 · {time}", "dynamic.refreshPipelines": "파이프라인을 새로고침했습니다.", "dynamic.refreshRepositories": "저장소를 새로고침했습니다.", "dynamic.refreshRunners": "Runner 상태를 새로고침했습니다.", "dynamic.refreshGraphs": "파이프라인 그래프를 새로고침했습니다.", "dynamic.beforeRun": "{branch} · 실행 전", "dynamic.viewRun": "실행 보기", "dynamic.noRuns": "실행 이력 없음", "dynamic.idle": "대기", "dynamic.copyUrl": "URL 복사", "dynamic.runPipeline": "파이프라인 실행", "dynamic.delete": "삭제", "dynamic.urlCopied": "저장소 URL을 복사했습니다.", "dynamic.copyFailed": "클립보드에 복사하지 못했습니다.", "dynamic.repositoryCreated": "저장소를 생성했습니다.", "dynamic.repositoryDeleted": "저장소를 삭제했습니다.", "dynamic.nameMismatch": "저장소 이름이 일치해야 합니다.", "dynamic.tokenCopied": "Lore 액세스 토큰을 복사했습니다.", "dynamic.pipelineQueued": "새 파이프라인을 큐에 추가했습니다.", "dynamic.starting": "시작 중…", "dynamic.loading": "불러오는 중…", "dynamic.loadingLogs": "로그를 불러오는 중…", "dynamic.workerPreparing": "Worker가 작업 구성을 준비하고 있습니다.", "dynamic.noLogs": "아직 출력된 로그가 없습니다.", "dynamic.logsPruned": "보존 기간이 지난 로그가 삭제되었습니다. 남아 있는 출력은 아래에 표시됩니다.\n", "dynamic.cancelRequested": "파이프라인 취소를 요청했습니다.",
    "dynamic.successRate": "성공률 {rate}", "dynamic.successRateEmpty": "성공률 —", "dynamic.countPipelines": "파이프라인 {count}개", "dynamic.countFilteredPipelines": "파이프라인 {shown} / {total}개", "dynamic.countRoutes": "경로 {count}개", "dynamic.countFilteredRoutes": "경로 {shown} / {total}개", "dynamic.countRepositories": "저장소 {count}개", "dynamic.countFilteredRepositories": "저장소 {shown} / {total}개", "dynamic.countRunners": "Runner {count}개", "dynamic.countFilteredRunners": "Runner {shown} / {total}개", "dynamic.pipelineTitle": "파이프라인 #{id}", "dynamic.jobs": "작업 {count}개", "dynamic.matchingPaths": "일치 경로 {count}개", "dynamic.pathRule": "경로 규칙", "dynamic.changedFolder": "변경 폴더", "dynamic.runsIn": "{directory}에서 실행", "dynamic.repositoryRoot": "저장소 루트", "dynamic.assigned": "{os} · 배정됨", "dynamic.target": "{os} 대상", "dynamic.waiting": "{os} 대기 중", "dynamic.stage": "단계 · {status}", "dynamic.graphAria": "{patterns} 변경이 {pipeline} 파이프라인과 {os} Runner를 거쳐 {stages} 단계를 실행합니다.", "dynamic.greetingMorning": "좋은 아침입니다, {name}님.", "dynamic.greetingAfternoon": "안녕하세요, {name}님.", "dynamic.greetingEvening": "좋은 저녁입니다, {name}님.", "dynamic.detail": "상세 보기",
    "status.configured": "설정됨", "status.queued": "대기 중", "status.running": "실행 중", "status.succeeded": "성공", "status.failed": "실패", "status.canceled": "취소됨", "status.skipped": "건너뜀", "status.online": "온라인", "status.offline": "오프라인"
  },
  "zh-CN": {
    "Create .lore-ci.toml": "创建 .lore-ci.toml",
    "Use template": "使用模板",
    "This repository has no CI configuration. Start with a build template, edit the commands, then save.": "此仓库没有 CI 配置。使用构建模板，修改命令后保存。",
    "CI settings are unavailable for Lore link source repositories.": "作为其他仓库 Lore 链接源的仓库无法使用 CI 设置。",
    "Select a repository and open CI settings to create a pipeline run.": "选择仓库，然后在 CI 设置中创建流水线运行。",
    "Selected branch is no longer available. Refresh CI settings.": "所选分支已不存在。请刷新 CI 设置。",
    "Language": "语言", "워크스페이스를 불러오는 중…": "正在加载工作区…", "LoreHub 홈": "LoreHub 首页", "LORE NATIVE DEV PLATFORM": "LORE 原生开发平台", "코드에서 배포까지,": "从代码到部署，", "하나의 흐름으로.": "尽在一个工作流。", "Lore revision을 안전하게 실행하고 파이프라인의 모든 단계를 한곳에서 확인하세요.": "安全运行 Lore 修订，并在一处查看流水线的每个阶段。", "ZENOGRID WORKSPACE": "ZENOGRID 工作区", "LoreHub에 로그인": "登录 LoreHub", "조직 계정으로 로그인해 프로젝트와 CI 파이프라인을 관리합니다.": "使用组织账号登录并管理项目与 CI 流水线。", "Google 계정으로 계속": "使用 Google 账号继续", "@zenogrid.co.kr 계정만 사용할 수 있습니다": "仅支持 @zenogrid.co.kr 账号", "Lore VCS 기반 개발 플랫폼": "基于 Lore VCS 的开发平台",
    "주 메뉴": "主菜单", "Overview": "概览", "Pipelines": "流水线", "Graphs": "图表", "Repositories": "仓库", "Runners": "Runner", "Workspace": "工作区", "Internal": "内部", "Coordinator online": "Coordinator 在线", "파이프라인 검색": "搜索流水线", "Search pipelines…": "搜索流水线…", "새로고침": "刷新", "로그아웃": "退出登录", "CI workspace": "CI 工作区", "현재 워크스페이스의 파이프라인 상태입니다.": "当前工作区的流水线状态。", "New pipeline": "新建流水线", "파이프라인 요약": "流水线摘要", "Total pipelines": "流水线总数", "최근 100건": "最近 100 条", "Succeeded": "成功", "성공률 —": "成功率 —", "Active": "运行中", "Queued + running": "排队 + 运行中", "Needs attention": "需要关注", "Failed pipelines": "失败的流水线", "Recent pipelines": "最近的流水线", "최신 Lore revision 실행 내역": "最新 Lore 修订运行记录", "상태 필터": "状态筛选", "All": "全部", "Finished": "已完成", "Status": "状态", "Pipeline": "流水线", "Revision": "修订", "Created": "创建时间", "Duration": "耗时", "Actions": "操作", "파이프라인이 없습니다": "暂无流水线", "Lore repository와 revision을 지정해 첫 실행을 시작하세요.": "选择 Lore 仓库和修订以开始首次运行。", "업데이트 대기 중": "等待更新",
    "CI routing": "CI 路由", "Pipeline graphs": "流水线图表", "폴더 변경이 어떤 파이프라인과 Runner, stage를 실행하는지 확인합니다.": "查看每个文件夹变更会触发哪个流水线、Runner 和阶段。", "Pipeline graph 요약": "流水线图表摘要", "Routes": "路由", "최근 pipeline별 경로": "当前修订的路由", "Folder rules": "文件夹规则", "고유한 changes 규칙": "唯一变更规则", "Runner targets": "Runner 目标", "대상 운영체제": "目标操作系统", "Folder routing map": "文件夹路由图", "각 pipeline의 가장 최근 실행 snapshot을 표시합니다": "显示每条流水线的最近运行快照", "표시할 routing graph가 없습니다": "暂无路由图", "changes와 runner_os가 있는 pipeline이 실행되면 여기에 표시됩니다.": "包含 changes 和 runner_os 的流水线会显示在此处。", "그래프를 선택하면 실행 상세를 엽니다.": "选择图表以打开运行详情。",
    "Lore server": "Lore 服务器", "현재 Lore 서버의 repository를 생성하고 연결 주소를 관리합니다.": "在当前 Lore 服务器上创建仓库并管理连接地址。", "New repository": "新建仓库", "Current Lore server": "当前 Lore 服务器", "CLI access token": "CLI 访问令牌", "Online": "在线", "Repository list": "仓库列表", "서버에서 조회한 Lore repository": "从服务器加载的 Lore 仓库", "Repository가 없습니다": "暂无仓库", "현재 Lore 서버에 첫 repository를 만드세요.": "在当前 Lore 服务器上创建第一个仓库。", "Google Workspace 로그인 사용자만 관리할 수 있습니다.": "仅已登录的 Google Workspace 用户可以管理仓库。",
    "CI infrastructure": "CI 基础设施", "등록된 runner의 운영체제, 연결 상태와 현재 작업을 확인합니다.": "查看已注册 Runner 的操作系统、连接状态和当前任务。", "Runner 다운로드": "下载 Runner", "LoreHub runner와 Lore CLI, 설치 스크립트가 포함된 x86_64 패키지입니다.": "包含 LoreHub Runner、Lore CLI 和安装脚本的 x86_64 软件包。", "Runner 요약": "Runner 摘要", "Total runners": "Runner 总数", "등록된 전체 runner": "所有已注册 Runner", "15초 내 heartbeat": "15 秒内收到心跳", "Offline": "离线", "종료 또는 heartbeat 만료": "已停止或心跳过期", "Registered runners": "已注册的 Runner", "5초 간격으로 상태가 갱신됩니다": "状态每 5 秒刷新", "Runner": "Runner", "Operating system": "操作系统", "Architecture": "架构", "Version": "版本", "Last seen": "最后在线", "Current job": "当前任务", "등록된 runner가 없습니다": "暂无已注册 Runner", "Linux 또는 Windows에서 lorehub worker를 실행하면 자동으로 표시됩니다.": "在 Linux 或 Windows 上运行 lorehub worker 后会自动显示。", "Offline runner도 이력으로 유지됩니다.": "离线 Runner 仍保留在历史记录中。",
    "RUN CI": "运行 CI", "닫기": "关闭", "실행할 Lore 저장소와 변경되지 않는 64자리 revision hash를 입력하세요.": "输入要运行的 Lore 仓库和不可变的 64 位修订哈希。", "Repository URL": "仓库 URL", "lores:// 주소": "lores:// 地址", "64-character Lore revision hash": "64 位 Lore 修订哈希", "branch나 head 대신 전체 revision hash": "使用完整修订哈希，而不是分支或 head", "Cancel": "取消", "Run pipeline": "运行流水线", "LORE SERVER": "LORE 服务器", "현재 서버에 새로운 Lore repository를 생성합니다.": "在当前服务器上创建新的 Lore 仓库。", "Repository name": "仓库名称", "영문자·숫자로 시작하고 끝나며 점, 대시, 밑줄 사용 가능": "以字母或数字开头和结尾，可使用点、连字符和下划线。", "Description": "描述", "optional": "可选", "Repository purpose": "仓库用途", "Create repository": "创建仓库", "DANGER ZONE": "危险操作", "Delete repository": "删除仓库", "repository와 서버에 저장된 데이터를 삭제합니다. 이 작업은 되돌릴 수 없습니다.": "删除仓库及服务器上的数据。此操作无法撤销。", "확인을 위해 repository 이름 입력": "输入仓库名称以确认", "CLI AUTHENTICATION": "CLI 身份验证", "Lore access token": "Lore 访问令牌", "Google 로그인 계정으로 발급된 토큰입니다. 1시간 동안 유효하며 다른 사람과 공유하지 마세요.": "这是为您的 Google 账号签发的令牌，有效期一小时，请勿与他人共享。", "Access token": "访问令牌", "Copy token": "复制令牌", "Close": "关闭", "PIPELINE": "流水线", "Execution graph": "执行图", "folder routing": "文件夹路由", "Jobs": "任务", "Job log": "任务日志", "live": "实时", "로그를 불러오는 중…": "正在加载日志…", "Cancel pipeline": "取消流水线",
    "dynamic.errorUser": "无法加载登录信息。", "dynamic.sessionExpired": "会话已过期。", "dynamic.requestFailed": "请求失败（{status}）。", "dynamic.updated": "刚刚更新 · {time}", "dynamic.refreshPipelines": "流水线已刷新。", "dynamic.refreshRepositories": "仓库已刷新。", "dynamic.refreshRunners": "Runner 状态已刷新。", "dynamic.refreshGraphs": "流水线图表已刷新。", "dynamic.beforeRun": "{branch} · 尚未运行", "dynamic.viewRun": "查看运行", "dynamic.noRuns": "尚无运行", "dynamic.idle": "空闲", "dynamic.copyUrl": "复制 URL", "dynamic.runPipeline": "运行流水线", "dynamic.delete": "删除", "dynamic.urlCopied": "仓库 URL 已复制。", "dynamic.copyFailed": "无法复制到剪贴板。", "dynamic.repositoryCreated": "仓库已创建。", "dynamic.repositoryDeleted": "仓库已删除。", "dynamic.nameMismatch": "仓库名称必须一致。", "dynamic.tokenCopied": "Lore 访问令牌已复制。", "dynamic.pipelineQueued": "新流水线已加入队列。", "dynamic.starting": "正在启动…", "dynamic.loading": "正在加载…", "dynamic.loadingLogs": "正在加载日志…", "dynamic.workerPreparing": "Worker 正在准备任务配置。", "dynamic.noLogs": "暂无日志输出。", "dynamic.logsPruned": "已按保留策略删除过期日志。剩余输出显示在下方。\n", "dynamic.cancelRequested": "已请求取消流水线。",
    "dynamic.successRate": "成功率 {rate}", "dynamic.successRateEmpty": "成功率 —", "dynamic.countPipelines": "{count} 条流水线", "dynamic.countFilteredPipelines": "{shown} / {total} 条流水线", "dynamic.countRoutes": "{count} 条路由", "dynamic.countFilteredRoutes": "{shown} / {total} 条路由", "dynamic.countRepositories": "{count} 个仓库", "dynamic.countFilteredRepositories": "{shown} / {total} 个仓库", "dynamic.countRunners": "{count} 个 Runner", "dynamic.countFilteredRunners": "{shown} / {total} 个 Runner", "dynamic.pipelineTitle": "流水线 #{id}", "dynamic.jobs": "{count} 个任务", "dynamic.matchingPaths": "{count} 个匹配路径", "dynamic.pathRule": "路径规则", "dynamic.changedFolder": "变更文件夹", "dynamic.runsIn": "在 {directory} 中运行", "dynamic.repositoryRoot": "仓库根目录", "dynamic.assigned": "{os} · 已分配", "dynamic.target": "目标 {os}", "dynamic.waiting": "等待 {os}", "dynamic.stage": "阶段 · {status}", "dynamic.graphAria": "{patterns} 的变更会通过 {os} Runner 运行流水线 {pipeline} 的 {stages} 阶段。", "dynamic.greetingMorning": "早上好，{name}。", "dynamic.greetingAfternoon": "下午好，{name}。", "dynamic.greetingEvening": "晚上好，{name}。", "dynamic.detail": "查看详情",
    "status.configured": "已配置", "status.queued": "排队中", "status.running": "运行中", "status.succeeded": "成功", "status.failed": "失败", "status.canceled": "已取消", "status.skipped": "已跳过", "status.online": "在线", "status.offline": "离线"
  }
};

Object.assign(I18N.en, {
  "document.title": "LoreHub · CI workspace",
  "테마": "Theme", "theme.system": "System", "theme.light": "Light", "theme.dark": "Dark",
  "search.pipelines.placeholder": "Search pipelines…", "search.pipelines.label": "Search pipelines",
  "search.graphs.placeholder": "Search graphs…", "search.graphs.label": "Search graphs",
  "search.repositories.placeholder": "Search repositories…", "search.repositories.label": "Search repositories",
  "search.runners.placeholder": "Search Runners…", "search.runners.label": "Search Runners",
  "Runner OS": "Runner OS", "Any OS": "Any OS", "dynamic.profile": "{name} profile",
  "Sparse View": "Sparse View", "View rules": "View rules", "dynamic.noSparseView": "No View", "Category": "Category", "dynamic.uncategorized": "Uncategorized", "dynamic.countCategories": "{count} categories", "dynamic.viewSnapshot": "build-time snapshot",
  "dynamic.openPipeline": "Open {repository} pipeline details", "dynamic.invalidLoreUrl": "Enter a lores:// URL.",
  "dynamic.creating": "Creating…", "unit.second": "{count}s", "unit.minuteSecond": "{minutes}m {seconds}s",
  "dynamic.jobs.one": "{count} job", "dynamic.jobs.other": "{count} jobs",
  "dynamic.matchingPaths.one": "{count} matching path", "dynamic.matchingPaths.other": "{count} matching paths",
  "dynamic.countPipelines.one": "{count} pipeline", "dynamic.countPipelines.other": "{count} pipelines",
  "dynamic.countRoutes.one": "{count} route", "dynamic.countRoutes.other": "{count} routes",
  "dynamic.countBranches.one": "{count} branch", "dynamic.countBranches.other": "{count} branches",
  "dynamic.countRepositories.one": "{count} repository", "dynamic.countRepositories.other": "{count} repositories",
  "dynamic.countRunners.one": "{count} Runner", "dynamic.countRunners.other": "{count} Runners",
  "LoreHub runner와 Lore CLI, 설치 도구가 포함된 플랫폼별 패키지입니다.": "Platform packages include LoreHub Runner, Lore CLI, and installation tools.",
  "Linux, Windows 또는 macOS에서 lorehub worker를 실행하면 자동으로 표시됩니다.": "Run lorehub worker on Linux, Windows, or macOS and it will appear automatically.",
  "Offline Runner는 작업 메뉴에서 등록 해제할 수 있습니다.": "Offline Runners can be removed from the Actions menu.",
  "Remove Runner": "Remove Runner",
  "Runner의 등록 정보를 삭제합니다. Runner 프로그램과 작업 파일은 유지되며, Runner가 다시 실행되면 자동으로 등록됩니다.": "Remove the Runner registration. The program and work files remain, and the Runner registers again when restarted.",
  "확인을 위해 Runner 이름 입력": "Enter the Runner name to confirm",
  "dynamic.remove": "Remove", "dynamic.removeRunner": "Remove {name}",
  "dynamic.stopRunnerFirst": "Stop the Runner before removing it.",
  "dynamic.runnerNameMismatch": "The Runner name must match.",
  "dynamic.runnerRemoved": "Runner registration removed.",
  "runner.pause": "Pause assignments", "runner.resume": "Resume assignments",
  "runner.draining": "Finishing current work", "runner.drained": "Maintenance mode",
  "runner.pauseHint": "Stop assigning new work to {name}. Current work will finish normally.",
  "runner.resumeHint": "Allow {name} to receive new work again.",
  "runner.paused": "New assignments paused. Current work will finish normally.",
  "runner.resumed": "New assignments resumed.", "runner.restricted": "Running (restricted)",
  "배정 중지는 현재 작업을 마친 뒤 새 작업을 받지 않습니다. 점검 후 배정 재개를 누르세요.": "Pausing assignments lets current work finish and blocks new work. Resume assignments after maintenance.",
  "docker.installed": "Installed", "docker.notInstalled": "Not installed", "docker.unknown": "Not reported",
  "Sources and runtime": "Sources and runtime", "Management": "Management", "Execution graphs": "Execution graphs", "CI settings": "CI settings",
  "Repository branch tree": "Repository branch tree", "Repository와 branch별로 pipeline 실행 그래프를 구분합니다.": "Groups pipeline execution graphs by repository and branch.",
  "파이프라인 상세 필터": "Pipeline filters", "All repositories": "All repositories", "All branches": "All branches", "All pipelines": "All pipelines", "Reset filters": "Reset filters",
  "dynamic.manualRun": "Manual run",
  "워크스페이스의 파이프라인 실행 이력과 현재 상태를 확인합니다.": "Review pipeline run history and current status for the workspace.",
  "실행할 Lore 저장소와 branch를 선택하세요.": "Choose the Lore repository and branch to run.", "Repository": "Repository", "Branch": "Branch", "Pipeline": "Pipeline", "현재 Lore 서버에 등록된 저장소": "Repositories registered on the current Lore server", "main branch가 기본으로 선택됩니다.": "The main branch is selected by default.", "선택한 branch의 최신 revision": "Latest revision of the selected branch", "선택한 revision의 .lore-ci.toml에 정의된 pipeline": "Pipeline defined in .lore-ci.toml at the selected revision", "dynamic.loadingBranches": "Loading branches…", "dynamic.loadingPipelines": "Loading pipelines…", "dynamic.defaultPipeline": "Default pipeline", "dynamic.noRepositories": "No repositories available", "dynamic.noBranches": "No active branches available", "dynamic.noPipelines": "No pipelines available",
  "Pipeline branches": "Pipeline branches", "저장소에서": "in the repository", "자동 실행을 감지할 branch를 선택하세요.": "Choose the branches monitored for automatic runs.", "선택하지 않으면 이 저장소의 자동 CI가 중지됩니다. 기존 실행 이력은 유지됩니다.": "Selecting no branches pauses automatic CI for this repository. Existing run history is kept.", "Save branches": "Save branches", "dynamic.pipelineBranches": "Auto CI branches", "dynamic.branchPolicySaved": "Automatic CI branches saved.", "dynamic.noRemoteBranches": "No active remote branches are available.",
  "CI CONFIGURATION": "CI CONFIGURATION", "저장소의 branch별 CI 설정을 확인하고 편집합니다.": "View and edit the repository's CI configuration by branch.", "저장소와 branch별 CI 파이프라인을 편집합니다.": "Edit CI pipelines by repository and branch.", "선택한 저장소의 branch별 CI 설정을 확인하고 편집합니다.": "View and edit CI configuration for each branch of the selected repository.", "유효한 TOML만 저장되며 저장 시 새 Lore revision이 생성됩니다.": "Only valid TOML can be saved. Saving creates a new Lore revision.", "CI configuration": "CI configuration", "Edit": "Edit", "Save changes": "Save changes", "dynamic.loadingConfig": "Loading .lore-ci.toml…", "dynamic.noCiConfig": ".lore-ci.toml does not exist on this branch. Select Edit to create it.", "dynamic.configSaved": ".lore-ci.toml saved to a new revision.", "dynamic.saving": "Saving…", "dynamic.configRefreshed": "CI configuration refreshed.", "dynamic.discardConfig": "Discard unsaved CI configuration changes?",
  "Visual": "Visual", "Pipeline list": "Pipeline list", "Stages run from left to right": "Stages run from left to right", "Manual pipeline": "Manual pipeline", "Manual": "Manual", "No CI configuration": "No CI configuration", "Select Edit to create a pipeline graph.": "Select Edit to create a pipeline graph.", "Nothing selected": "Nothing selected", "Select a pipeline, stage, or job.": "Select a pipeline, stage, or job.", "Pipeline settings": "Pipeline settings", "Stage settings": "Stage settings", "Job settings": "Job settings", "Name": "Name", "Stage": "Stage", "Timeout (seconds)": "Timeout (seconds)", "Script": "Script", "one command per line": "one command per line", "Working directory": "Working directory", "Change paths": "Change paths", "one path per line": "one path per line", "Add pipeline": "Add pipeline", "Add stage": "Add stage", "Add job": "Add job", "Delete pipeline": "Delete pipeline", "Delete stage": "Delete stage", "Delete job": "Delete job", "Convert to auto pipeline": "Convert to auto pipeline", "This is a manual pipeline using root stages and jobs.": "This is a manual pipeline using root stages and jobs.", "dynamic.jobSteps": "{count} commands · {seconds}s"
});
Object.assign(I18N.ko, {
  "document.title": "LoreHub · CI 워크스페이스",
  "테마": "테마", "theme.system": "시스템", "theme.light": "라이트", "theme.dark": "다크",
  "search.pipelines.placeholder": "파이프라인 검색…", "search.pipelines.label": "파이프라인 검색",
  "search.graphs.placeholder": "그래프 검색…", "search.graphs.label": "그래프 검색",
  "search.repositories.placeholder": "저장소 검색…", "search.repositories.label": "저장소 검색",
  "search.runners.placeholder": "Runner 검색…", "search.runners.label": "Runner 검색",
  "Runner OS": "Runner 운영체제", "Any OS": "모든 운영체제", "dynamic.profile": "{name} 프로필",
  "Sparse View": "Sparse View", "View rules": "View 규칙", "dynamic.noSparseView": "View 없음", "Category": "카테고리", "dynamic.uncategorized": "미분류", "dynamic.countCategories": "카테고리 {count}개", "dynamic.viewSnapshot": "빌드 시점 스냅샷",
  "dynamic.openPipeline": "{repository} 파이프라인 상세 열기", "dynamic.invalidLoreUrl": "lores:// 주소를 입력하세요.",
  "dynamic.creating": "생성 중…", "unit.second": "{count}초", "unit.minuteSecond": "{minutes}분 {seconds}초",
  "Remove Runner": "Runner 등록 해제",
  "dynamic.remove": "등록 해제", "dynamic.removeRunner": "{name} 등록 해제",
  "dynamic.stopRunnerFirst": "Runner를 중지한 후 등록 해제할 수 있습니다.",
  "dynamic.runnerNameMismatch": "Runner 이름이 일치해야 합니다.",
  "dynamic.runnerRemoved": "Runner 등록을 해제했습니다.",
  "runner.pause": "배정 중지", "runner.resume": "배정 재개",
  "runner.draining": "작업 마무리 중", "runner.drained": "유지보수 모드",
  "runner.pauseHint": "{name}의 새 작업 배정을 중지합니다. 현재 작업은 정상적으로 마무리합니다.",
  "runner.resumeHint": "{name}이 새 작업을 다시 받을 수 있도록 합니다.",
  "runner.paused": "새 작업 배정을 중지했습니다. 현재 작업은 계속 실행됩니다.",
  "runner.resumed": "새 작업 배정을 재개했습니다.", "runner.restricted": "실행 중 (접근 제한)",
  "docker.installed": "설치됨", "docker.notInstalled": "미설치", "docker.unknown": "확인 전",
  "dynamic.countBranches": "Branch {count}개",
  "Sources and runtime": "소스 및 실행 환경", "Management": "관리", "Execution graphs": "실행 그래프", "CI settings": "CI 설정",
  "Repository branch tree": "리포지토리 Branch 트리", "Repository와 branch별로 pipeline 실행 그래프를 구분합니다.": "리포지토리와 branch별로 파이프라인 실행 그래프를 구분합니다.",
  "파이프라인 상세 필터": "파이프라인 상세 필터", "All repositories": "전체 리포지토리", "All branches": "전체 Branch", "All pipelines": "전체 파이프라인", "Reset filters": "필터 초기화",
  "dynamic.manualRun": "수동 실행",
  "실행할 Lore 저장소와 branch를 선택하세요.": "실행할 Lore 저장소와 branch를 선택하세요.", "Repository": "저장소", "Branch": "Branch", "Pipeline": "파이프라인", "현재 Lore 서버에 등록된 저장소": "현재 Lore 서버에 등록된 저장소", "main branch가 기본으로 선택됩니다.": "main branch가 기본으로 선택됩니다.", "선택한 branch의 최신 revision": "선택한 branch의 최신 revision", "선택한 revision의 .lore-ci.toml에 정의된 pipeline": "선택한 revision의 .lore-ci.toml에 정의된 파이프라인", "dynamic.loadingBranches": "Branch 불러오는 중…", "dynamic.loadingPipelines": "파이프라인 불러오는 중…", "dynamic.defaultPipeline": "기본 파이프라인", "dynamic.noRepositories": "사용 가능한 저장소가 없습니다", "dynamic.noBranches": "사용 가능한 branch가 없습니다", "dynamic.noPipelines": "사용 가능한 파이프라인이 없습니다",
  "Pipeline branches": "파이프라인 Branch", "저장소에서": "저장소에서", "자동 실행을 감지할 branch를 선택하세요.": "자동 실행을 감지할 branch를 선택하세요.", "선택하지 않으면 이 저장소의 자동 CI가 중지됩니다. 기존 실행 이력은 유지됩니다.": "선택하지 않으면 이 저장소의 자동 CI가 중지됩니다. 기존 실행 이력은 유지됩니다.", "Save branches": "Branch 저장", "dynamic.pipelineBranches": "자동 CI Branch", "dynamic.branchPolicySaved": "자동 CI branch 설정을 저장했습니다.", "dynamic.noRemoteBranches": "사용 가능한 remote branch가 없습니다.",
  "CI CONFIGURATION": "CI 설정", "저장소의 branch별 CI 설정을 확인하고 편집합니다.": "저장소의 branch별 CI 설정을 확인하고 편집합니다.", "저장소와 branch별 CI 파이프라인을 편집합니다.": "저장소와 branch별 CI 파이프라인을 편집합니다.", "선택한 저장소의 branch별 CI 설정을 확인하고 편집합니다.": "선택한 저장소의 branch별 CI 설정을 확인하고 편집합니다.", "유효한 TOML만 저장되며 저장 시 새 Lore revision이 생성됩니다.": "유효한 TOML만 저장되며 저장 시 새 Lore revision이 생성됩니다.", "CI configuration": "CI 설정", "Edit": "편집", "Save changes": "변경 사항 저장", "dynamic.loadingConfig": ".lore-ci.toml 불러오는 중…", "dynamic.noCiConfig": "이 branch에 .lore-ci.toml이 없습니다. 편집을 선택해 새로 만드세요.", "dynamic.configSaved": ".lore-ci.toml을 새 revision으로 저장했습니다.", "dynamic.saving": "저장 중…", "dynamic.configRefreshed": "CI 설정을 새로고침했습니다.", "dynamic.discardConfig": "저장하지 않은 CI 설정 변경 사항을 버릴까요?",
  "Visual": "시각화", "Pipeline list": "파이프라인 목록", "Stages run from left to right": "단계는 왼쪽에서 오른쪽으로 실행됩니다", "Manual pipeline": "수동 파이프라인", "Manual": "수동", "No CI configuration": "CI 설정 없음", "Select Edit to create a pipeline graph.": "편집을 선택해 파이프라인 그래프를 만드세요.", "Nothing selected": "선택 항목 없음", "Select a pipeline, stage, or job.": "파이프라인, 단계 또는 작업을 선택하세요.", "Pipeline settings": "파이프라인 설정", "Stage settings": "단계 설정", "Job settings": "작업 설정", "Name": "이름", "Stage": "단계", "Timeout (seconds)": "제한 시간(초)", "Script": "스크립트", "one command per line": "한 줄에 명령 하나", "Working directory": "작업 디렉터리", "Change paths": "변경 경로", "one path per line": "한 줄에 경로 하나", "Add pipeline": "파이프라인 추가", "Add stage": "단계 추가", "Add job": "작업 추가", "Delete pipeline": "파이프라인 삭제", "Delete stage": "단계 삭제", "Delete job": "작업 삭제", "Convert to auto pipeline": "자동 파이프라인으로 전환", "This is a manual pipeline using root stages and jobs.": "루트 stages와 jobs를 사용하는 수동 파이프라인입니다.", "dynamic.jobSteps": "명령 {count}개 · {seconds}초"
});
Object.assign(I18N["zh-CN"], {
  "document.title": "LoreHub · CI 工作区",
  "테마": "主题", "theme.system": "跟随系统", "theme.light": "浅色", "theme.dark": "深色",
  "search.pipelines.placeholder": "搜索流水线…", "search.pipelines.label": "搜索流水线",
  "search.graphs.placeholder": "搜索图表…", "search.graphs.label": "搜索图表",
  "search.repositories.placeholder": "搜索仓库…", "search.repositories.label": "搜索仓库",
  "search.runners.placeholder": "搜索 Runner…", "search.runners.label": "搜索 Runner",
  "Runner OS": "Runner 操作系统", "Any OS": "任意操作系统", "dynamic.profile": "{name} 的头像",
  "Sparse View": "稀疏视图", "View rules": "视图规则", "dynamic.noSparseView": "无视图", "Category": "类别", "dynamic.uncategorized": "未分类", "dynamic.countCategories": "{count} 个类别", "dynamic.viewSnapshot": "构建时快照",
  "dynamic.openPipeline": "打开 {repository} 流水线详情", "dynamic.invalidLoreUrl": "请输入 lores:// 地址。",
  "dynamic.creating": "正在创建…", "unit.second": "{count}秒", "unit.minuteSecond": "{minutes}分 {seconds}秒",
  "LoreHub runner와 Lore CLI, 설치 도구가 포함된 플랫폼별 패키지입니다.": "各平台安装包包含 LoreHub Runner、Lore CLI 和安装工具。",
  "Linux, Windows 또는 macOS에서 lorehub worker를 실행하면 자동으로 표시됩니다.": "在 Linux、Windows 或 macOS 上运行 lorehub worker 后，它会自动显示。",
  "Offline Runner는 작업 메뉴에서 등록 해제할 수 있습니다.": "可从操作菜单中移除离线 Runner。",
  "Remove Runner": "移除 Runner",
  "Runner의 등록 정보를 삭제합니다. Runner 프로그램과 작업 파일은 유지되며, Runner가 다시 실행되면 자동으로 등록됩니다.": "移除 Runner 注册信息。程序和工作文件会保留，Runner 再次启动时会自动注册。",
  "확인을 위해 Runner 이름 입력": "输入 Runner 名称以确认",
  "dynamic.remove": "移除", "dynamic.removeRunner": "移除 {name}",
  "dynamic.stopRunnerFirst": "请先停止 Runner，然后再将其移除。",
  "dynamic.runnerNameMismatch": "Runner 名称必须一致。",
  "dynamic.runnerRemoved": "Runner 注册已移除。",
  "runner.pause": "暂停分配", "runner.resume": "恢复分配",
  "runner.draining": "正在完成当前任务", "runner.drained": "维护模式",
  "runner.pauseHint": "停止向 {name} 分配新任务。当前任务将正常完成。",
  "runner.resumeHint": "允许 {name} 再次接收新任务。",
  "runner.paused": "已暂停新任务分配。当前任务继续执行。",
  "runner.resumed": "已恢复新任务分配。", "runner.restricted": "运行中（受限）",
  "배정 중지는 현재 작업을 마친 뒤 새 작업을 받지 않습니다. 점검 후 배정 재개를 누르세요.": "暂停分配后，当前任务正常完成，不再接收新任务。维护结束后请恢复分配。",
  "docker.installed": "已安装", "docker.notInstalled": "未安装", "docker.unknown": "尚未报告",
  "dynamic.countBranches": "{count} 个分支",
  "Sources and runtime": "源代码与运行环境", "Management": "管理", "Execution graphs": "执行图", "CI settings": "CI 设置",
  "Repository branch tree": "仓库分支树", "Repository와 branch별로 pipeline 실행 그래프를 구분합니다.": "按仓库和分支整理流水线执行图。",
  "파이프라인 상세 필터": "流水线详细筛选", "All repositories": "所有仓库", "All branches": "所有分支", "All pipelines": "所有流水线", "Reset filters": "重置筛选",
  "dynamic.manualRun": "手动运行",
  "워크스페이스의 파이프라인 실행 이력과 현재 상태를 확인합니다.": "查看工作区的流水线运行历史和当前状态。",
  "실행할 Lore 저장소와 branch를 선택하세요.": "选择要运行的 Lore 仓库和分支。", "Repository": "仓库", "Branch": "分支", "Pipeline": "流水线", "현재 Lore 서버에 등록된 저장소": "当前 Lore 服务器上注册的仓库", "main branch가 기본으로 선택됩니다.": "默认选择 main 分支。", "선택한 branch의 최신 revision": "所选分支的最新修订", "선택한 revision의 .lore-ci.toml에 정의된 pipeline": "所选修订中 .lore-ci.toml 定义的流水线", "dynamic.loadingBranches": "正在加载分支…", "dynamic.loadingPipelines": "正在加载流水线…", "dynamic.defaultPipeline": "默认流水线", "dynamic.noRepositories": "没有可用的仓库", "dynamic.noBranches": "没有可用的活动分支", "dynamic.noPipelines": "没有可用的流水线",
  "Pipeline branches": "流水线分支", "저장소에서": "仓库中", "자동 실행을 감지할 branch를 선택하세요.": "选择要监控自动运行的分支。", "선택하지 않으면 이 저장소의 자동 CI가 중지됩니다. 기존 실행 이력은 유지됩니다.": "如果不选择分支，此仓库的自动 CI 将暂停。现有运行历史会保留。", "Save branches": "保存分支", "dynamic.pipelineBranches": "自动 CI 分支", "dynamic.branchPolicySaved": "自动 CI 分支设置已保存。", "dynamic.noRemoteBranches": "没有可用的远程分支。",
  "CI CONFIGURATION": "CI 配置", "저장소의 branch별 CI 설정을 확인하고 편집합니다.": "按分支查看和编辑仓库的 CI 配置。", "저장소와 branch별 CI 파이프라인을 편집합니다.": "按仓库和分支编辑 CI 流水线。", "선택한 저장소의 branch별 CI 설정을 확인하고 편집합니다.": "查看和编辑所选仓库各分支的 CI 配置。", "유효한 TOML만 저장되며 저장 시 새 Lore revision이 생성됩니다.": "只能保存有效的 TOML；保存后会创建新的 Lore 修订。", "CI configuration": "CI 配置", "Edit": "编辑", "Save changes": "保存更改", "dynamic.loadingConfig": "正在加载 .lore-ci.toml…", "dynamic.noCiConfig": "此分支没有 .lore-ci.toml。选择编辑以创建。", "dynamic.configSaved": ".lore-ci.toml 已保存到新的修订。", "dynamic.saving": "正在保存…", "dynamic.configRefreshed": "CI 配置已刷新。", "dynamic.discardConfig": "要放弃未保存的 CI 配置更改吗？",
  "Visual": "可视化", "Pipeline list": "流水线列表", "Stages run from left to right": "阶段从左到右运行", "Manual pipeline": "手动流水线", "Manual": "手动", "No CI configuration": "无 CI 配置", "Select Edit to create a pipeline graph.": "选择编辑以创建流水线图。", "Nothing selected": "未选择项目", "Select a pipeline, stage, or job.": "请选择流水线、阶段或任务。", "Pipeline settings": "流水线设置", "Stage settings": "阶段设置", "Job settings": "任务设置", "Name": "名称", "Stage": "阶段", "Timeout (seconds)": "超时（秒）", "Script": "脚本", "one command per line": "每行一个命令", "Working directory": "工作目录", "Change paths": "变更路径", "one path per line": "每行一个路径", "Add pipeline": "添加流水线", "Add stage": "添加阶段", "Add job": "添加任务", "Delete pipeline": "删除流水线", "Delete stage": "删除阶段", "Delete job": "删除任务", "Convert to auto pipeline": "转换为自动流水线", "This is a manual pipeline using root stages and jobs.": "这是使用根级 stages 和 jobs 的手动流水线。", "dynamic.jobSteps": "{count} 条命令 · {seconds}秒"
});
Object.assign(I18N.en, { "Dependencies": "Dependencies", "Select jobs that must complete first.": "Select jobs that must complete first.", "No eligible dependency jobs": "No eligible dependency jobs", "dynamic.jobNeeds": "Needs {jobs}" });
Object.assign(I18N.ko, { "Dependencies": "의존 작업", "Select jobs that must complete first.": "먼저 완료되어야 하는 작업을 선택하세요.", "No eligible dependency jobs": "선택 가능한 의존 작업 없음", "dynamic.jobNeeds": "선행 작업 · {jobs}" });
Object.assign(I18N["zh-CN"], { "Dependencies": "依赖任务", "Select jobs that must complete first.": "选择必须先完成的任务。", "No eligible dependency jobs": "没有可选的依赖任务", "dynamic.jobNeeds": "依赖 {jobs}" });
Object.assign(I18N.en, { "Pipeline dependencies": "Pipeline dependencies", "Select pipelines that must succeed first.": "Select pipelines that must succeed first.", "No eligible dependency pipelines": "No eligible dependency pipelines", "dynamic.pipelineNeeds": "Depends on {pipelines}", "Waiting for pipeline dependencies": "Waiting for pipeline dependencies" });
Object.assign(I18N.ko, { "Pipeline dependencies": "파이프라인 의존성", "Select pipelines that must succeed first.": "먼저 성공해야 하는 파이프라인을 선택하세요.", "No eligible dependency pipelines": "선택 가능한 선행 파이프라인 없음", "dynamic.pipelineNeeds": "선행 파이프라인 · {pipelines}", "Waiting for pipeline dependencies": "선행 파이프라인 대기 중" });
Object.assign(I18N["zh-CN"], { "Pipeline dependencies": "流水线依赖", "Select pipelines that must succeed first.": "选择必须先成功的流水线。", "No eligible dependency pipelines": "没有可选的依赖流水线", "dynamic.pipelineNeeds": "依赖流水线 {pipelines}", "Waiting for pipeline dependencies": "正在等待依赖流水线" });
Object.assign(I18N.en, { "Failure reason": "Failure reason" });
Object.assign(I18N.ko, { "Failure reason": "실패 사유" });
Object.assign(I18N["zh-CN"], { "Failure reason": "失败原因" });
Object.assign(I18N.en, {
  "Repository links": "Repository links", "Lore dependencies": "Lore dependencies", "저장소 branch가 참조하는 다른 Lore 저장소의 경로와 pin을 관리합니다.": "Manage paths and pins from other Lore repositories referenced by a repository branch.",
  "Add link": "Add link", "Linked paths": "Linked paths", "선택한 root revision에 기록된 repository link": "Repository links recorded in the selected root revision", "Repository link가 없습니다": "No repository links", "다른 저장소의 경로를 이 branch에 연결하세요.": "Connect a path from another repository to this branch.", "변경 사항은 새 Lore revision으로 commit 후 push됩니다.": "Changes are committed and pushed as a new Lore revision.",
  "LORE DEPENDENCY": "LORE DEPENDENCY", "Add repository link": "Add repository link", "의 선택한 branch에 다른 저장소 경로를 연결합니다.": "Connect a path from another repository to the selected branch.", "Link path": "Link path", "현재 저장소 안에 생성될 상대 경로": "Relative path created in the current repository", "Source repository": "Source repository", "Source branch": "Source branch", "Source path": "Source path", "Source 저장소의 상대 경로이며 전체 root는": "Relative path in the source repository; use", "을 사용합니다.": "for the entire root.", "Disable linked branch creation": "Disable linked branch creation", "Link creation failed": "Link creation failed",
  "dynamic.refreshRepositoryLinks": "Repository links refreshed.", "dynamic.countLinks": "{count} links", "dynamic.updateLink": "Update to latest", "dynamic.removeLink": "Remove", "dynamic.linkAdded": "Repository link added and pushed.", "dynamic.linkAddedWithSourceFolder": "The missing source folder was created and pushed before adding the repository link.", "dynamic.linkUpdated": "Repository link updated and pushed.", "dynamic.linkRemoved": "Repository link removed and pushed.", "dynamic.removeLinkConfirm": "Remove repository link {path}? This creates and pushes a new revision.", "dynamic.loadingLinks": "Loading repository links…", "dynamic.noSourceRepositories": "No other accessible repositories are available.", "dynamic.sourceUnavailable": "Repository {id}", "dynamic.pinnedRevision": "Pinned revision", "dynamic.tracking": "Branch creation enabled", "dynamic.fixed": "Branch creation disabled"
});
Object.assign(I18N.ko, {
  "Repository links": "저장소 링크", "Lore dependencies": "Lore 의존성", "Add link": "링크 추가", "Linked paths": "연결된 경로", "Repository link가 없습니다": "저장소 링크가 없습니다", "다른 저장소의 경로를 이 branch에 연결하세요.": "다른 저장소의 경로를 이 브랜치에 연결하세요.", "Add repository link": "저장소 링크 추가", "Link path": "링크 경로", "Source repository": "Source 저장소", "Source branch": "Source branch", "Source path": "Source 경로", "Disable linked branch creation": "연결된 branch 자동 생성을 사용하지 않음", "Link creation failed": "링크 생성 실패",
  "dynamic.refreshRepositoryLinks": "저장소 링크를 새로고침했습니다.", "dynamic.countLinks": "링크 {count}개", "dynamic.updateLink": "최신 revision으로 갱신", "dynamic.removeLink": "제거", "dynamic.linkAdded": "저장소 링크를 추가하고 push했습니다.", "dynamic.linkAddedWithSourceFolder": "없는 Source 폴더를 생성하고 commit·push한 뒤 저장소 링크를 추가했습니다.", "dynamic.linkUpdated": "저장소 링크를 갱신하고 push했습니다.", "dynamic.linkRemoved": "저장소 링크를 제거하고 push했습니다.", "dynamic.removeLinkConfirm": "저장소 링크 {path}을(를) 제거할까요? 새 revision이 생성되어 push됩니다.", "dynamic.loadingLinks": "저장소 링크를 불러오는 중…", "dynamic.noSourceRepositories": "접근 가능한 다른 저장소가 없습니다.", "dynamic.sourceUnavailable": "저장소 {id}", "dynamic.pinnedRevision": "고정 revision", "dynamic.tracking": "Branch 생성 허용", "dynamic.fixed": "Branch 생성 비활성"
});
Object.assign(I18N["zh-CN"], {
  "Repository links": "仓库链接", "Lore dependencies": "Lore 依赖", "저장소 branch가 참조하는 다른 Lore 저장소의 경로와 pin을 관리합니다.": "管理仓库分支引用的其他 Lore 仓库路径和固定修订。", "Add link": "添加链接", "Linked paths": "已链接路径", "선택한 root revision에 기록된 repository link": "所选根修订中记录的仓库链接", "Repository link가 없습니다": "没有仓库链接", "다른 저장소의 경로를 이 branch에 연결하세요.": "将其他仓库中的路径连接到此分支。", "변경 사항은 새 Lore revision으로 commit 후 push됩니다.": "更改将作为新的 Lore 修订提交并推送。", "Add repository link": "添加仓库链接", "Link path": "链接路径", "Source repository": "源仓库", "Source branch": "源分支", "Source path": "源路径", "Disable linked branch creation": "禁用链接分支创建", "Link creation failed": "链接创建失败",
  "dynamic.refreshRepositoryLinks": "仓库链接已刷新。", "dynamic.countLinks": "{count} 个链接", "dynamic.updateLink": "更新到最新", "dynamic.removeLink": "移除", "dynamic.linkAdded": "仓库链接已添加并推送。", "dynamic.linkAddedWithSourceFolder": "缺失的源文件夹已创建并提交推送，然后添加了仓库链接。", "dynamic.linkUpdated": "仓库链接已更新并推送。", "dynamic.linkRemoved": "仓库链接已移除并推送。", "dynamic.removeLinkConfirm": "要移除仓库链接 {path} 吗？这会创建并推送新修订。", "dynamic.loadingLinks": "正在加载仓库链接…", "dynamic.noSourceRepositories": "没有其他可访问的仓库。", "dynamic.sourceUnavailable": "仓库 {id}", "dynamic.pinnedRevision": "固定修订", "dynamic.tracking": "允许创建分支", "dynamic.fixed": "禁用分支创建"
});

function initialLocale() {
  const saved = window.localStorage.getItem("lorehub_locale");
  if (SUPPORTED_LOCALES.includes(saved)) return saved;
  const preferred = navigator.language.toLowerCase();
  if (preferred.startsWith("ko")) return "ko";
  if (preferred.startsWith("zh")) return "zh-CN";
  return "en";
}

function initialTheme() {
  const initial = document.documentElement.dataset.theme;
  return SUPPORTED_THEMES.includes(initial) ? initial : "system";
}

for (const [locale, label] of [["ko", "파일·폴더"], ["en", "Files and folders"], ["zh-CN", "文件与文件夹"]]) I18N[locale]["Folder view"] = label;

for (const [key, labels] of Object.entries({
  "Files and folders": ["파일·폴더", "Files and folders", "文件与文件夹"],
  "All run history": ["전체 실행 이력", "All run history", "全部运行历史"],
  "Run history": ["실행 이력", "Run history", "运行历史"],
  "Selected repository": ["선택한 저장소", "Selected repository", "所选仓库"],
  "Links": ["링크", "Links", "链接"],
})) ["ko", "en", "zh-CN"].forEach((locale, index) => { I18N[locale][key] = labels[index]; });

function t(key, values = {}) {
  const template = I18N[state.locale]?.[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (_, name) => values[name] ?? "");
}

function tc(key, count, values = {}) {
  const category = new Intl.PluralRules(localeTag()).select(count);
  const variant = `${key}.${category}`;
  const resolved = Object.prototype.hasOwnProperty.call(I18N[state.locale], variant) ? variant : key;
  return t(resolved, { count, ...values });
}

const LINK_LABELS = {
  "Source changes flow into Root. Manage sync policies and recover unfinished links.": ["Source 변경을 Root에 반영합니다. 동기화 정책과 미완료 링크를 관리하세요.", "将 Source 更改同步到 Root，管理同步策略并恢复未完成的链接。"],
  "Root repository": ["Root 저장소", "Root 仓库"], "Root branch": ["Root 브랜치", "Root 分支"],
  "Source · read from": ["Source · 가져올 원본", "Source · 读取来源"], "Root · link into": ["Root · 연결할 대상", "Root · 链接目标"],
  "Link creation history": ["링크 생성 이력", "链接创建历史"],
  "Source commits are preserved when Root creation fails. Retry to finish the link.": ["Root 연결 실패 시 Source commit은 유지됩니다. 재시도로 연결을 완료하세요.", "Root 创建失败时保留 Source 提交。重试以完成链接。"],
  "Create missing Source folder, commit and push": ["없는 Source 폴더를 생성하고 commit · push", "创建缺失的 Source 文件夹并提交、推送"],
  "Sync policy": ["동기화 정책", "同步策略"], "Automatic sync": ["자동 동기화", "自动同步"], "Manual sync": ["수동 동기화", "手动同步"],
  "Automatic sync commits and pushes Source updates to this Root branch.": ["자동 동기화는 Source 변경을 이 Root 브랜치에 commit · push합니다.", "自动同步会将 Source 更新提交并推送到此 Root 分支。"],
  "Advanced Lore options": ["고급 Lore 옵션", "高级 Lore 选项"],
  "This controls Lore branching, not automatic synchronization.": ["Lore의 브랜치 생성 옵션이며 자동 동기화 설정과는 별개입니다.", "此选项控制 Lore 分支创建，与自动同步无关。"],
  "Up to date": ["최신", "已是最新"], "Update available": ["업데이트 있음", "有可用更新"], "Sync failed": ["동기화 실패", "同步失败"],
  "Source unavailable": ["Source 확인 불가", "无法访问 Source"], "Last successful sync": ["마지막 성공", "上次成功同步"],
  "No sync recorded": ["기록 없음", "暂无记录"], "Technical details": ["기술 정보", "技术详情"], "Latest Source revision": ["Source 최신 revision", "Source 最新 revision"],
  "Enable automatic sync": ["자동 동기화 켜기", "启用自动同步"], "Switch to manual sync": ["수동 동기화로 전환", "切换为手动同步"],
  "Sync policy saved": ["동기화 정책을 저장했습니다.", "同步策略已保存"], "No creation history": ["생성 이력이 없습니다.", "暂无创建历史"],
  "Validate": ["검증", "验证"], "Prepare Source": ["Source 준비", "准备 Source"], "Create Root link": ["Root 링크 생성", "创建 Root 链接"], "Complete": ["완료", "完成"],
  "In progress": ["진행 중", "进行中"], "Creation failed": ["생성 실패", "创建失败"], "Source ready · Root incomplete": ["Source 준비 완료 · Root 미완료", "Source 已就绪 · Root 未完成"],
  "Retry Root link": ["Root 링크 재시도", "重试 Root 链接"], "Retry creation": ["생성 재시도", "重试创建"],
  "Retry checks the latest Root revision and preserves existing Source commits. Continue?": ["최신 Root revision을 확인하고 Source commit을 유지한 채 재시도합니다. 계속할까요?", "将检查最新 Root revision 并保留现有 Source 提交后重试。是否继续？"],
  "Request interrupted. Refresh the creation history before retrying.": ["요청이 중단되었습니다. 생성 이력을 새로고침하여 상태를 확인하세요.", "请求中断。请刷新创建历史后再重试。"],
  "Source folder committed and pushed": ["Source 폴더 commit · push 완료", "Source 文件夹已提交并推送"],
  "Source verified": ["Source 확인 완료", "Source 已验证"], "Refresh to check progress": ["새로고침으로 진행 상황 확인", "刷新以查看进度"],
  "Expand folder": ["폴더 펼치기", "展开文件夹"], "Collapse folder": ["폴더 접기", "折叠文件夹"],
  "Link details": ["링크 상세", "链接详情"], "Link actions": ["링크 작업", "链接操作"],
  "Retry": ["다시 시도", "重试"], "No files or folders": ["파일이나 폴더가 없습니다.", "没有文件或文件夹。"],
  "Loading folder…": ["폴더를 불러오는 중…", "正在加载文件夹…"],
  "Delete link": ["링크 삭제", "删除链接"], "Only linked folders can be deleted here.": ["여기서는 LINK 폴더만 삭제할 수 있습니다.", "这里只能删除 LINK 文件夹。"],
  "A LINK cannot be created below {path} because it is already a LINK.": ["{path}은(는) LINK 노드이므로 그 아래에 LINK를 만들 수 없습니다.", "{path} 已是 LINK 节点，无法在其下创建 LINK。"]
};
for (const [key, [ko, zh]] of Object.entries(LINK_LABELS)) {
  I18N.en[key] = key; I18N.ko[key] = ko; I18N["zh-CN"][key] = zh;
}

const state = {
  locale: initialLocale(),
  theme: initialTheme(),
  user: null,
  pipelines: [],
  pipelineNextBefore: null,
  pipelineRequest: 0,
  pipelineLoading: false,
  pipelineSearchTimer: null,
  pipelineHasOlderPages: false,
  repositoryScope: "",
  selectedRepository: "",
  repositoryBranch: "",
  repositories: [],
  runners: [],
  pipelineGraphs: [],
  graphExpandedRepositories: new Set(),
  graphExpandedBranches: new Set(),
  graphExpandedCategories: new Set(),
  repositoryServerUrl: "",
  repositoryStorageBackends: ["dynamodb_s3"],
  ciSettingsRepositories: [],
  section: "overview",
  repositoryToDelete: null,
  repositoryBranchesName: null,
  repositoryConfigName: null,
  repositoryConfigRevision: null,
  repositoryConfigContent: null,
  repositoryConfigModel: null,
  repositoryConfigDraft: null,
  repositoryConfigMode: "visual",
  repositoryConfigSelection: null,
  repositoryConfigEditing: false,
  repositoryConfigSaving: false,
  repositoryConfigStatus: "idle",
  repositoryConfigError: "",
  repositoryConfigRequest: 0,
  repositoryLinksName: null,
  repositoryLinksBranch: null,
  repositoryLinksRevision: null,
  repositoryLinks: [],
  repositoryLinkCollapsedPaths: new Set(),
  repositoryLinkOpenPaths: new Set(),
  repositoryLinkContent: new Map(),
  repositoryLinkSelectedPath: null,
  repositoryLinksStatus: "idle",
  repositoryLinksError: "",
  repositoryLinksRequest: 0,
  repositoryLinksBusy: false,
  repositoryLinkOperations: [],
  repositoryLinkOperationsError: "",
  repositoryLinkOperationsRequest: 0,
  repositoryLinkDraft: null,
  runnerToRemove: null,
  filter: "all",
  pipelineRepositoryFilter: "",
  pipelineBranchFilter: "",
  pipelineNameFilter: "",
  query: "",
  selectedId: null,
  detailLogView: null,
  refreshTimer: null,
  updatedAt: {},
};

const elements = {};
const localizedTextNodes = [];
const localizedAttributes = [];

document.addEventListener("DOMContentLoaded", () => {
  for (const id of [
    "loading-view", "login-view", "app-view", "pipeline-search", "refresh-button",
    "user-menu-button", "user-menu", "theme-select", "logout-button", "user-name", "user-email",
    "user-picture", "user-initials", "welcome-heading", "ci-new-pipeline-button",
    "ci-config-template", "ci-config-template-source", "ci-config-use-template",
    "new-pipeline-dialog", "pipeline-form", "repository-url", "branch", "revision", "pipeline-name", "run-pipeline-button",
    "pipeline-table-body", "empty-state", "pipeline-count", "load-more-pipelines", "last-updated",
    "pipeline-repository-filter", "pipeline-branch-filter", "pipeline-name-filter", "pipeline-filter-reset",
    "pipeline-detail-dialog", "detail-repository", "detail-title", "detail-summary",
    "execution-graph-section", "execution-graph", "job-count", "job-list", "pipeline-log", "cancel-pipeline-button", "toast-region",
    "detail-log-more", "detail-log-follow", "detail-log-restart", "detail-log-note", "detail-retry",
    "detail-copy-link", "detail-permalink",
    "repositories-page", "new-repository-button", "new-repository-dialog", "repository-form",
    "repository-name", "repository-description", "create-repository-button", "repository-list",
    "repository-empty-state", "repository-count", "nav-repository-count", "repository-server-url",
    "repository-last-updated", "delete-repository-dialog", "delete-repository-form",
    "delete-repository-name", "delete-repository-confirmation", "confirm-delete-repository-button",
    "repository-branches-dialog", "repository-branches-form", "repository-branches-name",
    "repository-branches-list", "save-repository-branches-button",
    "ci-settings-page", "repository-config-form", "repository-config-repository",
    "repository-config-branch", "repository-config-revision", "repository-config-viewer",
    "repository-config-editor-field", "repository-config-editor", "repository-config-cancel-edit",
    "repository-config-edit", "repository-config-save",
    "repository-config-mode", "repository-config-visual-tab", "repository-config-toml-tab",
    "repository-config-visual", "repository-config-pipeline-count", "repository-config-pipeline-list",
    "repository-config-add-pipeline", "repository-config-graph-title", "repository-config-stage-graph",
    "repository-config-inspector-title", "repository-config-inspector",
    "repository-links-page", "repository-link-repository",
    "repository-link-branch", "repository-link-revision", "repository-link-count",
    "repository-link-list", "repository-link-last-updated",
    "new-repository-link-dialog", "repository-link-form", "repository-link-root-name",
    "repository-link-path", "repository-link-source-repository", "repository-link-source-branch",
    "repository-link-source-path", "repository-link-disable-branching", "repository-link-form-error", "create-repository-link-button",
    "repository-link-create-source", "repository-link-auto-update", "repository-link-preview", "repository-link-progress", "repository-link-operations",
    "create-lore-token-button", "lore-token-dialog", "lore-access-token", "copy-lore-token-button",
    "runners-page", "runner-table-body", "runner-empty-state", "runner-count", "nav-runner-count",
    "runner-stat-total", "runner-stat-online", "runner-stat-offline", "runner-last-updated",
    "remove-runner-dialog", "remove-runner-form", "remove-runner-name",
    "remove-runner-confirmation", "confirm-remove-runner-button",
    "runner-diagnostics-dialog", "runner-diagnostics-title", "runner-diagnostics-status",
    "runner-diagnostics-hint", "runner-diagnostics-facts", "runner-diagnostics-freshness",
    "graphs-page", "pipeline-graph-list", "graph-empty-state", "graph-count", "nav-graph-count",
    "graph-stat-routes", "graph-stat-folders", "graph-stat-runners", "graph-last-updated",
  ]) elements[id] = document.getElementById(id);

  initManagement();
  initRepositoryContext();
  registerStaticTranslations();
  bindLocaleControls();
  bindThemeControl();
  applyLocale(false);
  applyTheme();
  bindEvents();
  void initialize();
});

function hasTranslation(key) {
  return SUPPORTED_LOCALES.some((locale) => Object.prototype.hasOwnProperty.call(I18N[locale], key));
}

function registerStaticTranslations() {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const node = walker.currentNode;
    if (["SCRIPT", "STYLE", "OPTION", "CODE"].includes(node.parentElement?.tagName)) continue;
    const key = node.nodeValue.trim();
    if (!key || !hasTranslation(key)) continue;
    const leading = node.nodeValue.match(/^\s*/)?.[0] || "";
    const trailing = node.nodeValue.match(/\s*$/)?.[0] || "";
    localizedTextNodes.push({ node, key, leading, trailing });
  }
  for (const element of document.querySelectorAll("[placeholder], [aria-label], [title]")) {
    for (const attribute of ["placeholder", "aria-label", "title"]) {
      const key = element.getAttribute(attribute);
      if (key && hasTranslation(key)) localizedAttributes.push({ element, attribute, key });
    }
  }
}

function bindLocaleControls() {
  for (const select of document.querySelectorAll(".language-select")) {
    select.value = state.locale;
    select.addEventListener("change", () => {
      state.locale = SUPPORTED_LOCALES.includes(select.value) ? select.value : "en";
      window.localStorage.setItem("lorehub_locale", state.locale);
      applyLocale(true);
    });
  }
}

function bindThemeControl() {
  elements["theme-select"].value = state.theme;
  elements["theme-select"].addEventListener("change", () => {
    state.theme = SUPPORTED_THEMES.includes(elements["theme-select"].value)
      ? elements["theme-select"].value
      : "system";
    try {
      window.localStorage.setItem("lorehub_theme", state.theme);
    } catch (_) {
      // The active selection still applies when browser storage is unavailable.
    }
    applyTheme();
  });
  THEME_MEDIA_QUERY.addEventListener("change", () => {
    if (state.theme === "system") applyTheme();
  });
}

function applyTheme() {
  const effectiveTheme = state.theme === "system"
    ? (THEME_MEDIA_QUERY.matches ? "dark" : "light")
    : state.theme;
  document.documentElement.dataset.theme = state.theme;
  document.documentElement.dataset.colorScheme = effectiveTheme;
  elements["theme-select"].value = state.theme;
  document.querySelector('meta[name="theme-color"]').content = effectiveTheme === "dark" ? "#15131d" : "#17142f";
}

function applyLocale(rerender) {
  document.documentElement.lang = state.locale;
  document.title = t("document.title");
  for (const select of document.querySelectorAll(".language-select")) select.value = state.locale;
  for (const option of elements["theme-select"].options) option.textContent = t(`theme.${option.value}`);
  for (const option of elements["repository-link-auto-update"].options) option.textContent = t(option.value === "true" ? "Automatic sync" : "Manual sync");
  for (const entry of localizedTextNodes) entry.node.nodeValue = `${entry.leading}${t(entry.key)}${entry.trailing}`;
  for (const entry of localizedAttributes) entry.element.setAttribute(entry.attribute, t(entry.key));
  managementLocale();
  renderRepositoryContext();
  document.querySelector('.status-tab[data-status="failed"]').textContent = t("status.failed");
  if (!rerender) return;
  if (state.user) renderUser(state.user);
  updateSectionSearch();
  renderPipelines();
  renderOverview();
  renderRepositories();
  renderRunners();
  renderPipelineGraphs();
  renderUpdatedLabels();
  if (state.section === "ci-settings") renderRepositoryConfig();
  if (state.section === "repository-links") renderRepositoryLinks();
  if (state.section === "repository-tree") renderRepositoryTree();
  if (state.selectedId && elements["pipeline-detail-dialog"].open) void loadPipelineDetail(state.selectedId);
}

function bindEvents() {
  document.querySelectorAll('a[href="/auth/google/login"]').forEach(link => link.addEventListener("click", rememberPipelineLogin));
  elements["new-repository-button"].addEventListener("click", openNewRepository);
  document.querySelectorAll(".js-open-repository").forEach((button) => button.addEventListener("click", openNewRepository));
  elements["ci-new-pipeline-button"].addEventListener("click", () => {
    if (elements["ci-new-pipeline-button"].disabled) return;
    void openNewPipeline(state.repositoryScope, state.repositoryBranch);
  });
  elements["ci-config-use-template"].addEventListener("click", () => {
    if (state.repositoryConfigStatus === "ready" && state.repositoryConfigContent === null) setRepositoryConfigEditing(true);
  });
  document.querySelectorAll(".modal-close, .modal-cancel").forEach((button) => button.addEventListener("click", () => elements["new-pipeline-dialog"].close()));
  document.querySelectorAll(".drawer-close").forEach((button) => button.addEventListener("click", () => elements["pipeline-detail-dialog"].close()));
  elements["pipeline-detail-dialog"].addEventListener("close", () => {
    if (elements["pipeline-detail-dialog"].open) return;
    const id = state.selectedId;
    executionDetailRequest++;
    state.selectedId = null;
    state.detailLogView = null;
    if (id) closePipelineLocation(id);
  });
  elements["detail-copy-link"].addEventListener("click", async () => {
    if (!state.selectedId) return;
    try { await navigator.clipboard.writeText(pipelinePermalink(state.selectedId)); toast(t("dynamic.urlCopied"), "success"); }
    catch (_) { toast(t("dynamic.copyFailed"), "error"); }
  });
  elements["detail-log-more"].addEventListener("click", () => void loadPipelineLogs(state.selectedId));
  elements["detail-log-restart"].addEventListener("click", () => void restartDetailLogs());
  elements["detail-log-follow"].addEventListener("click", () => {
    const cache = state.detailLogView;
    if (!cache) return;
    cache.follow = !cache.follow;
    renderLogs(cache.rows, cache.prunedAt);
    if (cache.follow && !cache.more) void loadPipelineLogs(cache.id);
  });
  elements["pipeline-log"].addEventListener("scroll", () => {
    const cache = state.detailLogView, terminal = elements["pipeline-log"];
    if (!cache?.follow || terminal.scrollTop + terminal.clientHeight >= terminal.scrollHeight - 8) return;
    cache.follow = false;
    renderLogs(cache.rows, cache.prunedAt);
  });
  elements["detail-retry"].addEventListener("click", () => { if (state.selectedId) void openPipeline(state.selectedId); });
  document.querySelectorAll(".repository-modal-close, .repository-modal-cancel").forEach((button) => button.addEventListener("click", () => elements["new-repository-dialog"].close()));
  document.querySelectorAll(".delete-modal-close, .delete-modal-cancel").forEach((button) => button.addEventListener("click", () => elements["delete-repository-dialog"].close()));
  document.querySelectorAll(".repository-branches-modal-close, .repository-branches-modal-cancel").forEach((button) => button.addEventListener("click", () => elements["repository-branches-dialog"].close()));
  document.querySelectorAll(".runner-remove-modal-close, .runner-remove-modal-cancel").forEach((button) => button.addEventListener("click", () => elements["remove-runner-dialog"].close()));
  elements["pipeline-form"].addEventListener("submit", submitPipeline);
  elements["repository-form"].addEventListener("submit", createRepository);
  elements["delete-repository-form"].addEventListener("submit", deleteRepository);
  elements["repository-branches-form"].addEventListener("submit", saveRepositoryPipelineBranches);
  elements["repository-config-form"].addEventListener("submit", saveRepositoryConfig);
  elements["repository-config-repository"].addEventListener("change", () => chooseRepositorySection("ci-settings", elements["repository-config-repository"].value));
  elements["repository-config-branch"].addEventListener("change", () => void loadRepositoryConfig());
  elements["repository-config-edit"].addEventListener("click", () => setRepositoryConfigEditing(true));
  elements["repository-config-cancel-edit"].addEventListener("click", () => setRepositoryConfigEditing(false));
  elements["repository-config-visual-tab"].addEventListener("click", () => void setRepositoryConfigMode("visual"));
  elements["repository-config-toml-tab"].addEventListener("click", () => void setRepositoryConfigMode("toml"));
  elements["repository-config-add-pipeline"].addEventListener("click", addVisualPipeline);
  document.querySelectorAll(".repository-link-modal-close, .repository-link-modal-cancel").forEach((button) => button.addEventListener("click", () => elements["new-repository-link-dialog"].close()));
  elements["repository-link-form"].addEventListener("submit", createRepositoryLink);
  elements["repository-link-repository"].addEventListener("change", () => chooseRepositorySection("repository-links", elements["repository-link-repository"].value));
  elements["repository-link-branch"].addEventListener("change", () => void loadRepositoryLinks());
  elements["repository-link-source-repository"].addEventListener("change", () => void loadRepositoryLinkSourceBranches());
  elements["repository-link-list"].addEventListener("contextmenu", openRepositoryLinkContextMenu);
  document.addEventListener("pointerdown", (event) => {
    if (repositoryLinkContextMenu && !repositoryLinkContextMenu.contains(event.target)) closeRepositoryLinkContextMenu();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && repositoryLinkContextMenu) closeRepositoryLinkContextMenu(true);
  });
  window.addEventListener("scroll", () => closeRepositoryLinkContextMenu(), true);
  window.addEventListener("resize", () => closeRepositoryLinkContextMenu());
  window.addEventListener("hashchange", () => closeRepositoryLinkContextMenu());
  elements["repository-link-operations"].addEventListener("click", retryRepositoryLink);
  elements["repository-link-form"].addEventListener("input", renderRepositoryLinkPreview);
  window.addEventListener("resize", () => {
    if (state.section !== "ci-settings" || state.repositoryConfigMode !== "visual") return;
    const model = state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel;
    const selected = selectedCiPipeline(model);
    if (selected) window.requestAnimationFrame(() => renderConfigDependencyEdges(selected.pipeline));
  });
  elements["remove-runner-form"].addEventListener("submit", removeRunner);
  elements["create-lore-token-button"].addEventListener("click", issueLoreToken);
  elements["copy-lore-token-button"].addEventListener("click", copyLoreToken);
  document.querySelectorAll(".token-modal-close").forEach((button) => button.addEventListener("click", closeLoreToken));
  elements["repository-url"].addEventListener("input", () => elements["repository-url"].setCustomValidity(""));
  elements.branch.addEventListener("change", () => void selectPipelineBranch());
  elements["repository-url"].addEventListener("change", () => void loadPipelineBranches());
  elements["pipeline-search"].addEventListener("input", (event) => {
    state.query = event.target.value.trim().toLowerCase();
    if (state.section === "repositories") renderRepositories();
    else if (state.section === "runners") renderRunners();
    else if (state.section === "graphs") renderPipelineGraphs();
    else if (isManagement()) { if (!management.dirty) renderManagement(); }
    else searchPipelineHistory(250);
  });
  elements["refresh-button"].addEventListener("click", () => void refreshSection(true));
  elements["load-more-pipelines"].addEventListener("click", () => void loadPipelines(false, true));
  elements["logout-button"].addEventListener("click", logout);
  elements["cancel-pipeline-button"].addEventListener("click", cancelPipeline);
  elements["user-menu-button"].addEventListener("click", () => {
    const expanded = elements["user-menu-button"].getAttribute("aria-expanded") === "true";
    elements["user-menu-button"].setAttribute("aria-expanded", String(!expanded));
    elements["user-menu"].hidden = expanded;
  });
  document.addEventListener("click", (event) => {
    if (!elements["user-menu-button"].contains(event.target) && !elements["user-menu"].contains(event.target)) {
      elements["user-menu-button"].setAttribute("aria-expanded", "false");
      elements["user-menu"].hidden = true;
    }
  });
  document.querySelectorAll(".status-tab").forEach((button) => button.addEventListener("click", () => {
    state.filter = button.dataset.status;
    document.querySelectorAll(".status-tab").forEach((tab) => tab.classList.toggle("is-active", tab === button));
    searchPipelineHistory();
  }));
  for (const [id, stateKey] of [
    ["pipeline-repository-filter", "pipelineRepositoryFilter"],
    ["pipeline-branch-filter", "pipelineBranchFilter"],
    ["pipeline-name-filter", "pipelineNameFilter"],
  ]) {
    elements[id].addEventListener(stateKey === "pipelineRepositoryFilter" ? "change" : "input", () => {
      if (stateKey === "pipelineRepositoryFilter") {
        navigateRepositorySection("pipelines", elements[id].value);
        return;
      }
      state[stateKey] = elements[id].value;
      searchPipelineHistory(250);
    });
  }
  elements["pipeline-filter-reset"].addEventListener("click", () => {
    state.pipelineRepositoryFilter = state.repositoryScope;
    state.pipelineBranchFilter = state.repositoryBranch || "";
    state.pipelineNameFilter = "";
    navigateRepositorySection("pipelines", state.repositoryScope);
  });
  elements["pipeline-table-body"].addEventListener("click", (event) => {
    const row = event.target.closest("tr[data-id]");
    if (row) void openPipeline(row.dataset.id);
  });
  elements["repository-list"].addEventListener("click", repositoryAction);
  elements["runner-table-body"].addEventListener("click", runnerAction);
  document.querySelectorAll(".nav-item[data-section]").forEach((link) => link.addEventListener("click", (event) => {
    event.preventDefault();
    const section = availableSections().includes(link.dataset.section) ? link.dataset.section : "overview";
    navigateRepositorySection(section, link.dataset.navScope === "repository" ? selectedRepository() : "");
  }));
  window.addEventListener("hashchange", () => void syncPipelineLocation());
  window.addEventListener("beforeunload", (event) => {
    if (!state.repositoryConfigEditing) return;
    event.preventDefault();
    event.returnValue = "";
  });
  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      elements["pipeline-search"].focus();
    }
  });
}

async function initialize() {
  try {
    const response = await fetch("/api/v1/me", { credentials: "same-origin" });
    if (response.status === 401) {
      showLogin();
      return;
    }
    if (!response.ok) throw new Error(t("dynamic.errorUser"));
    const user = await response.json();
    restorePipelineLogin();
    state.user = user;
    renderUser(user);
    elements["loading-view"].hidden = true;
    elements["app-view"].hidden = false;
    await Promise.all([loadRepositories(false), loadRunners(false)]);
    await syncPipelineLocation(true);
    state.refreshTimer = window.setInterval(() => void refreshActiveViews(), 5000);
  } catch (error) {
    showLogin();
    toast(error.message, "error");
  }
}

function sectionFromHash() {
  const section = repositoryRoute(window.location.hash).section;
  return availableSections().includes(section) ? section : "overview";
}

function availableSections() {
  return ["pipelines", "graphs", "repositories", "ci-settings", "repository-links", "repository-tree", "runners", ...(state.user?.role === "admin" ? MANAGEMENT_SECTIONS : [])];
}

async function showSection(section) {
  if (!availableSections().includes(section)) section = "overview";
  const route = repositoryRoute(window.location.hash);
  const scope = REPOSITORY_SECTIONS.includes(section) ? route.repository : "";
  if (!scope && ["repository-tree", "repository-links", "ci-settings", "graphs"].includes(section)) { navigateRepositorySection("repositories", ""); return; }
  const branch = scope ? route.branch || repositoryBranchSelections.get(scope) || "" : "";
  const branchChanged = branch !== (state.repositoryBranch || "");
  const scopeChanged = scope !== state.repositoryScope;
  if (state.section === section && !scopeChanged && !branchChanged && section === "ci-settings" && state.repositoryConfigEditing) return;
  if (state.section !== section && isManagement() && !discardManagement()) {
    history.replaceState(null, "", repositorySectionHash(state.section, state.repositoryScope));
    document.getElementById("mobile-page-select").value = repositoryNavigationValue();
    renderRepositoryContext();
    return;
  }
  if ((state.section !== section || scopeChanged || branchChanged) && state.section === "ci-settings" && !discardRepositoryConfigEdit()) {
    history.replaceState(null, "", repositorySectionHash(state.section, state.repositoryScope));
    document.getElementById("mobile-page-select").value = repositoryNavigationValue();
    renderRepositoryContext();
    return;
  }
  if (state.section !== section && state.section === "ci-settings") resetRepositoryConfig();
  if (scopeChanged || branchChanged) {
    state.repositoryScope = scope;
    state.pipelineRepositoryFilter = scope;
    state.pipelineBranchFilter = "";
    state.pipelineNameFilter = "";
    state.filter = "all";
    document.querySelectorAll(".status-tab").forEach(tab => tab.classList.toggle("is-active", tab.dataset.status === "all"));
    state.pipelineRequest++;
    state.pipelineLoading = false;
    state.pipelineHasOlderPages = false;
    state.pipelineNextBefore = null;
    state.pipelines = [];
    state.pipelineGraphs = [];
    executionGraphRequest++;
    state.repositoryLinksRequest++;
    state.repositoryConfigRequest++;
    state.repositoryConfigName = scope ? repositoryName(scope) : null;
    state.repositoryLinksName = scope ? repositoryName(scope) : null;
    renderPipelines();
    renderPipelineGraphs();
  }
  state.repositoryBranch = branch;
  if (scope) { repositoryBranchSelections.set(scope, branch); rememberRepositorySelection(scope); }
  repositoryNavigationRequest++;
  state.section = section;
  workspaceOverview.request++;
  workspaceOverview.loading = false;
  rememberPipelinePage(window.location.hash);
  renderRepositoryContext();
  document.getElementById("app-view").classList.toggle("has-repository-scope", Boolean(scope));
  for (const key of MANAGEMENT_SECTIONS) document.getElementById(`${key}-page`).hidden = section !== key;
  document.getElementById("mobile-page-select").value = repositoryNavigationValue();
  document.querySelectorAll(".overview-view").forEach((element) => { element.hidden = section !== "overview"; });
  document.querySelectorAll(".pipelines-view").forEach((element) => { element.hidden = section !== "pipelines"; });
  document.getElementById("pipelines").hidden = !["overview", "pipelines"].includes(section);
  elements["graphs-page"].hidden = section !== "graphs";
  elements["repositories-page"].hidden = section !== "repositories";
  elements["ci-settings-page"].hidden = section !== "ci-settings";
  elements["repository-links-page"].hidden = section !== "repository-links";
  document.getElementById("repository-tree-page").hidden = section !== "repository-tree";
  repositoryTree.request++;
  elements["runners-page"].hidden = section !== "runners";
  invalidatePipelineHistory();
  restorePipelineHistoryFilters(window.location.hash);
  renderPipelines();
  updateSectionSearch();
  if (scope && !(await loadRepositoryNavigationBranches())) return;
  restorePipelineHistoryFilters(window.location.hash);
  if (isManagement()) await loadManagement();
  else if (section === "repositories") await loadRepositories(false);
  else if (section === "ci-settings") await loadCiSettings(false);
  else if (section === "repository-links") await loadRepositoryLinksPage(state.repositoryLinksName);
  else if (section === "repository-tree") await loadRepositoryTreePage();
  else if (section === "runners") await loadRunners(false);
  else if (section === "graphs") await loadPipelineGraphs(false);
  else if (section === "overview") await Promise.all([loadOverview(), loadRepositories(false), loadPipelines(false)]);
  else if (section === "pipelines") await loadPipelines(false);
  else renderPipelines();
}

function updateSectionSearch() {
  if (["overview", "pipelines"].includes(state.section)) elements["pipeline-search"].maxLength = 256;
  else elements["pipeline-search"].removeAttribute("maxlength");
  if (state.section === "operations") {
    elements["pipeline-search"].disabled = true;
    elements["pipeline-search"].placeholder = ot("title");
    elements["pipeline-search"].previousElementSibling.textContent = ot("title");
    return;
  }
  elements["pipeline-search"].disabled = false;
  if (state.section === "overview") {
    elements["pipeline-search"].disabled = true;
    elements["pipeline-search"].placeholder = t("Overview");
    elements["pipeline-search"].previousElementSibling.textContent = t("Overview");
    return;
  }
  if (isManagement()) {
    const key = state.section === "workspace-views" ? "viewSearch" : state.section === "repository-access" ? "accessSearch" : "search";
    elements["pipeline-search"].placeholder = mt(key);
    elements["pipeline-search"].previousElementSibling.textContent = mt(key);
    return;
  }
  if (["ci-settings", "repository-links", "repository-tree"].includes(state.section)) {
    elements["pipeline-search"].disabled = true;
    const label = state.section === "repository-tree" ? rtt("title") : state.section === "ci-settings" ? t("CI configuration") : t("Repository links");
    elements["pipeline-search"].placeholder = label;
    elements["pipeline-search"].previousElementSibling.textContent = label;
    return;
  }
  const key = state.section === "repositories" ? "search.repositories" : state.section === "runners" ? "search.runners" : state.section === "graphs" ? "search.graphs" : "search.pipelines";
  elements["pipeline-search"].placeholder = t(`${key}.placeholder`);
  elements["pipeline-search"].previousElementSibling.textContent = t(`${key}.label`);
}

function renderUpdatedLabels() {
  const labels = {
    pipelines: "last-updated",
    repositories: "repository-last-updated",
    runners: "runner-last-updated",
    graphs: "graph-last-updated",
    repositoryLinks: "repository-link-last-updated",
  };
  for (const [section, elementId] of Object.entries(labels)) {
    const updatedAt = state.updatedAt[section];
    if (updatedAt) elements[elementId].textContent = t("dynamic.updated", { time: formatClock(updatedAt) });
  }
}

function showLogin() {
  elements["loading-view"].hidden = true;
  elements["login-view"].hidden = false;
}

function renderUser(user) {
  setManagementVisibility(user.role === "admin");
  const name = user.name || user.email.split("@")[0];
  elements["user-name"].textContent = name;
  elements["user-email"].textContent = user.email;
  elements["user-initials"].textContent = initials(name);
  elements["welcome-heading"].textContent = t("Overview");
  if (user.picture_url) {
    elements["user-picture"].src = user.picture_url;
    elements["user-picture"].alt = t("dynamic.profile", { name });
    elements["user-picture"].hidden = false;
    elements["user-initials"].hidden = true;
  }
}

async function api(path, options = {}) {
  const response = await fetch(path, { credentials: "same-origin", ...options });
  if (response.status === 401) {
    window.clearInterval(state.refreshTimer);
    window.location.reload();
    throw new Error(t("dynamic.sessionExpired"));
  }
  if (!response.ok) {
    let message = t("dynamic.requestFailed", { status: response.status });
    try { message = (await response.json()).error || message; } catch (_) { /* response has no JSON body */ }
    throw Object.assign(new Error(message), { status: response.status });
  }
  if (response.status === 204) return null;
  return response.json();
}

async function loadPipelines(notify, append = false) {
  if (state.pipelineLoading) return;
  if (append && state.section === "overview") return;
  const cursor = append ? state.pipelineNextBefore : null;
  if (append && !cursor) return;
  const scope = state.repositoryScope;
  const filters = pipelineHistoryParameters().toString();
  const request = ++state.pipelineRequest;
  state.pipelineLoading = true;
  elements["refresh-button"].disabled = true;
  elements["load-more-pipelines"].disabled = true;
  renderPipelines();
  try {
    const page = await api(`/api/v1/pipeline-history?limit=${state.section === "overview" ? 5 : 100}&${filters}${cursor ? `&before=${encodeURIComponent(cursor)}` : ""}`);
    if (request !== state.pipelineRequest || scope !== state.repositoryScope || filters !== pipelineHistoryParameters().toString()) return;
    state.pipelines = append ? [...state.pipelines, ...page.pipelines] : page.pipelines;
    state.pipelineNextBefore = page.next_before;
    state.pipelineHasOlderPages = append;
    renderPipelines();
    state.updatedAt.pipelines = new Date();
    renderUpdatedLabels();
    if (notify) toast(t("dynamic.refreshPipelines"), "success");
  } catch (error) {
    if (request !== state.pipelineRequest || scope !== state.repositoryScope || filters !== pipelineHistoryParameters().toString()) return;
    if (!append) { state.pipelines = []; state.pipelineNextBefore = null; renderPipelines(); }
    toast(error.message, "error");
  } finally {
    if (request === state.pipelineRequest && scope === state.repositoryScope) {
      state.pipelineLoading = false;
      elements["refresh-button"].disabled = false;
      elements["load-more-pipelines"].disabled = !state.pipelineNextBefore;
      elements["load-more-pipelines"].hidden = state.section === "overview" || !state.pipelineNextBefore;
      renderPipelines();
    }
  }
}

async function loadRepositories(notify) {
  elements["refresh-button"].disabled = true;
  repositoryListStatus = "loading";
  renderRepositoryContext();
  try {
    const payload = await api("/api/v1/repositories");
    state.repositories = payload.repositories;
    repositoryListStatus = "ready";
    reconcileRepositorySelection();
    state.repositoryServerUrl = payload.server_url;
    state.repositoryStorageBackends = payload.storage_backends ?? ["dynamodb_s3"];
    elements["repository-server-url"].textContent = payload.server_url;
    state.updatedAt.repositories = new Date();
    renderUpdatedLabels();
    if (notify) toast(t("dynamic.refreshRepositories"), "success");
  } catch (error) {
    repositoryListStatus = "error";
    toast(error.message, "error");
  } finally {
    // Replace the initial loading indicator even when the repository request fails.
    // Keep any previously loaded repositories visible while showing the error toast.
    renderRepositories();
    renderRepositoryContext();
    renderOverviewRepositories();
    elements["refresh-button"].disabled = false;
  }
}

let runnerListRequest = 0;
let runnerLoadError = false;
const runnerMutations = new Set();

async function loadRunners(notify) {
  if (runnerMutations.size) return;
  const request = ++runnerListRequest;
  elements["refresh-button"].disabled = true;
  try {
    const runners = await api("/api/v1/runners");
    if (request !== runnerListRequest) return;
    state.runners = runners;
    runnerLoadError = false;
    renderRunners();
    state.updatedAt.runners = new Date();
    renderUpdatedLabels();
    if (notify) toast(t("dynamic.refreshRunners"), "success");
  } catch (error) {
    if (request === runnerListRequest) {
      runnerLoadError = true;
      renderRunnerDiagnostics();
      toast(error.message, "error");
    }
  } finally {
    if (request === runnerListRequest) elements["refresh-button"].disabled = false;
  }
}

async function loadPipelineGraphs(notify) {
  const scope = state.repositoryScope;
  const request = ++executionGraphRequest;
  elements["refresh-button"].disabled = true;
  elements["pipeline-graph-list"].setAttribute("aria-busy", "true");
  try {
    const routes = await api(`/api/v1/pipeline-graphs?${repositoryQuery(scope).slice(1)}`);
    if (request !== executionGraphRequest || scope !== state.repositoryScope) return;
    const details = await Promise.all(routes.map(async (route) => {
      if (route.latest_pipeline_id) {
        const detail = await api(`/api/v1/pipelines/${encodeURIComponent(route.latest_pipeline_id)}`);
        return executionRouteDetail(route, detail);
      }
      const jobs = (route.graph?.stages || []).flatMap((stage, stageIndex) =>
        (stage.jobs || []).map((name, jobIndex) => ({
          id: `${stageIndex}-${jobIndex}`,
          name,
          stage: stage.name,
          status: "configured",
        }))
      );
      return {
        route,
        graph: route.graph,
        jobs,
        pipeline: {
          id: route.revision,
          repository_url: route.repository_url,
          revision: route.revision,
          revision_number: route.revision_number,
          pipeline_name: route.pipeline_name,
          category: route.category,
          runner_os: route.runner_os,
          branch: route.branch,
          trigger_patterns: route.trigger_patterns,
          changed_path_count: 0,
          working_directory: route.working_directory,
          sparse_view_name: route.graph?.sparse_view || null,
          status: "configured",
          worker_id: null,
          created_at: route.updated_at,
        },
      };
    }));
    if (request !== executionGraphRequest || scope !== state.repositoryScope) return;
    state.pipelineGraphs = details;
    renderPipelineGraphs();
    state.updatedAt.graphs = new Date();
    renderUpdatedLabels();
    if (notify) toast(t("dynamic.refreshGraphs"), "success");
  } catch (error) {
    if (request !== executionGraphRequest || scope !== state.repositoryScope) return;
    state.pipelineGraphs = [];
    renderPipelineGraphs();
    toast(error.message, "error");
  } finally {
    if (request === executionGraphRequest && scope === state.repositoryScope) {
      elements["pipeline-graph-list"].removeAttribute("aria-busy");
      elements["refresh-button"].disabled = false;
    }
  }
}

function pipelineCategory(pipeline) {
  return pipeline.category || t("dynamic.uncategorized");
}

function renderPipelineGraphs() {
  const details = state.pipelineGraphs.filter(({ pipeline }) => {
    if (state.repositoryScope && state.repositoryBranch && pipeline.branch !== state.repositoryBranch) return false;
    const terms = [
      pipeline.repository_url,
      pipeline.pipeline_name,
      pipeline.runner_os,
      pipeline.branch,
      pipelineCategory(pipeline),
      pipeline.status,
      pipeline.sparse_view_name,
      ...(pipeline.trigger_patterns || []),
    ];
    return !state.query || terms.some((value) => String(value).toLowerCase().includes(state.query));
  });
  const list = elements["pipeline-graph-list"];
  captureExecutionFocus(list);
  list.replaceChildren();
  list.hidden = details.length === 0;
  elements["graph-empty-state"].hidden = details.length !== 0;

  const repositories = new Map();
  for (const detail of details) {
    const repositoryUrl = detail.pipeline.repository_url;
    if (!repositories.has(repositoryUrl)) repositories.set(repositoryUrl, new Map());
    const branches = repositories.get(repositoryUrl);
    const branch = detail.pipeline.branch || "—";
    if (!branches.has(branch)) branches.set(branch, new Map());
    const categories = branches.get(branch);
    const category = pipelineCategory(detail.pipeline);
    if (!categories.has(category)) categories.set(category, []);
    categories.get(category).push(detail);
  }

  for (const [repositoryUrl, branches] of repositories) {
    const repositoryTree = document.createElement("details");
    repositoryTree.className = "graph-repository-tree";
    repositoryTree.open = Boolean(state.repositoryScope) || Boolean(state.query) || state.graphExpandedRepositories.has(repositoryUrl);
    repositoryTree.addEventListener("toggle", () => {
      if (repositoryTree.open) state.graphExpandedRepositories.add(repositoryUrl);
      else state.graphExpandedRepositories.delete(repositoryUrl);
    });

    const repositorySummary = document.createElement("summary");
    repositorySummary.className = "graph-repository-summary";
    const repositoryIdentity = document.createElement("span");
    repositoryIdentity.className = "graph-tree-identity";
    repositoryIdentity.append(
      textNode("›", "graph-tree-marker"),
      textNode("▱", "graph-tree-icon"),
      textNode(repositoryName(repositoryUrl), "graph-tree-title"),
    );
    const repositoryRouteCount = Array.from(branches.values()).reduce(
      (count, categories) => count + Array.from(categories.values()).reduce((total, pipelines) => total + pipelines.length, 0),
      0,
    );
    repositorySummary.append(
      repositoryIdentity,
      textNode(`${tc("dynamic.countBranches", branches.size)} · ${tc("dynamic.countRoutes", repositoryRouteCount)}`, "graph-tree-count"),
    );
    repositoryTree.append(repositorySummary);

    const branchList = document.createElement("div");
    branchList.className = "graph-branch-list";
    for (const [branch, categories] of branches) {
      const branchKey = `${repositoryUrl}\u0000${branch}`;
      const branchTree = document.createElement("details");
      branchTree.className = "graph-branch-tree";
      branchTree.open = Boolean(state.query) || state.graphExpandedBranches.has(branchKey);
      branchTree.addEventListener("toggle", () => {
        if (branchTree.open) state.graphExpandedBranches.add(branchKey);
        else state.graphExpandedBranches.delete(branchKey);
      });
      const branchSummary = document.createElement("summary");
      branchSummary.className = "graph-branch-summary";
      const branchIdentity = document.createElement("span");
      branchIdentity.className = "graph-tree-identity";
      branchIdentity.append(
        textNode("›", "graph-tree-marker"),
        textNode("⑂", "graph-tree-icon graph-tree-icon--branch"),
        textNode(branch, "graph-tree-title"),
      );
      const routeCount = Array.from(categories.values()).reduce((count, pipelines) => count + pipelines.length, 0);
      branchSummary.append(
        branchIdentity,
        textNode(`${t("dynamic.countCategories", { count: categories.size })} · ${tc("dynamic.countPipelines", routeCount)}`, "graph-tree-count"),
      );
      const categoryList = document.createElement("div");
      categoryList.className = "graph-category-list";
      for (const [category, pipelines] of categories) {
        const categoryKey = `${branchKey}\u0000${category}`;
        const categoryTree = document.createElement("details");
        categoryTree.className = "graph-category-tree";
        categoryTree.open = Boolean(state.query) || state.graphExpandedCategories.has(categoryKey);
        categoryTree.addEventListener("toggle", () => {
          if (categoryTree.open) state.graphExpandedCategories.add(categoryKey);
          else state.graphExpandedCategories.delete(categoryKey);
        });
        const categorySummary = document.createElement("summary");
        categorySummary.className = "graph-category-summary";
        const categoryIdentity = document.createElement("span");
        categoryIdentity.className = "graph-tree-identity";
        categoryIdentity.append(
          textNode("›", "graph-tree-marker"),
          textNode("◇", "graph-tree-icon graph-tree-icon--category"),
          textNode(category, "graph-tree-title"),
        );
        categorySummary.append(categoryIdentity, textNode(tc("dynamic.countPipelines", pipelines.length), "graph-tree-count"));
        const pipelineList = document.createElement("div");
        pipelineList.className = "graph-category-pipelines";
        for (const detail of pipelines) pipelineList.append(pipelineGraphCard(detail));
        categoryTree.append(categorySummary, pipelineList);
        categoryList.append(categoryTree);
      }
      branchTree.append(branchSummary, categoryList);
      branchList.append(branchTree);
    }
    repositoryTree.append(branchList);
    list.append(repositoryTree);
  }

  const routes = state.pipelineGraphs.length;
  const folderRules = new Set(state.pipelineGraphs.flatMap(({ pipeline }) => pipeline.trigger_patterns || [])).size;
  const runnerTargets = new Set(state.pipelineGraphs.map(({ pipeline }) => pipeline.runner_os).filter(Boolean)).size;
  elements["graph-stat-routes"].textContent = String(routes);
  elements["graph-stat-folders"].textContent = String(folderRules);
  elements["graph-stat-runners"].textContent = String(runnerTargets);
  elements["graph-count"].textContent = details.length === routes
    ? tc("dynamic.countRoutes", routes)
    : t("dynamic.countFilteredRoutes", { shown: details.length, total: routes });
  elements["nav-graph-count"].textContent = String(routes);
  elements["nav-graph-count"].hidden = routes === 0;
}

function pipelineGraphCard(detail) {
  const pipeline = detail.pipeline;
  const card = document.createElement("article");
  card.className = "pipeline-graph-card";
  const header = document.createElement("header");
  const identity = document.createElement("div");
  const eyebrow = document.createElement("p");
  eyebrow.className = "graph-card-eyebrow";
  eyebrow.textContent = pipelineCategory(pipeline);
  const title = document.createElement("h2");
  title.textContent = pipeline.pipeline_name;
  const metadata = document.createElement("div");
  metadata.className = "graph-card-meta";
  const runMeta = detail.route.latest_pipeline_id
    ? `${revisionLabel(pipeline)} · ${relativeTime(detail.route.latest_created_at)}`
    : t("dynamic.beforeRun", { branch: pipeline.branch });
  metadata.append(statusBadge(pipeline.status), osBadge(pipeline.runner_os));
  if (pipeline.sparse_view_name) metadata.append(viewBadge(pipeline.sparse_view_name));
  metadata.append(textNode(runMeta, "runner-meta"));
  identity.append(eyebrow, title, metadata);
  const actions = document.createElement("div");
  actions.className = "graph-card-actions";
  const run = document.createElement("button");
  run.type = "button";
  run.className = "button button--primary graph-run-button";
  run.textContent = t("Run current configuration");
  run.addEventListener("click", () => void runPipelineFromGraph(detail, run));
  const open = document.createElement("button");
  open.type = "button";
  open.className = "button button--ghost graph-open-button";
  open.textContent = detail.route.latest_pipeline_id ? t("dynamic.viewRun") : t("dynamic.noRuns");
  open.disabled = !detail.route.latest_pipeline_id;
  if (detail.route.latest_pipeline_id) open.addEventListener("click", () => void openPipeline(detail.route.latest_pipeline_id));
  actions.append(run, open);
  header.append(identity, actions);
  const graph = document.createElement("div");
  graph.className = "execution-graph graph-page-flow";
  renderExecutionWorkspace(graph, detail, `route:${JSON.stringify([detail.route.repository_url, detail.route.branch, detail.route.pipeline_name])}`);
  card.append(header, graph);
  return card;
}

async function runPipelineFromGraph(detail, button) {
  const route = detail.route;
  button.disabled = true;
  button.textContent = t("dynamic.starting");
  try {
    const pipeline = await api("/api/v1/pipelines", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({
        repository_url: route.repository_url,
        branch: route.branch,
        revision: route.revision,
        pipeline_name: route.pipeline_name,
      }),
    });
    toast(t("dynamic.pipelineQueued"), "success");
    await Promise.all([loadPipelines(false), loadPipelineGraphs(false)]);
    await openPipeline(pipeline.id);
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
    button.textContent = t("Run current configuration");
  }
}

let runnerDiagnosticId = null;
const runnerDiagnosticCopy = {
  title: ["Runner 진단", "Runner diagnostics", "Runner 诊断"],
  ready: ["작업 요청 정상", "Polling normally", "任务请求正常"],
  readyHint: ["최근 작업 요청을 받았습니다. 대기 작업이 실행되지 않으면 대상 OS와 선행 파이프라인 상태를 확인하세요.", "A recent work request was received. For queued work, check its target OS and upstream pipelines.", "已收到近期任务请求。如果任务仍在排队，请检查目标 OS 和上游流水线。"],
  starting: ["첫 작업 요청 대기", "Awaiting first poll", "等待首次任务请求"],
  startingHint: ["등록 이후 아직 작업 요청을 관측하지 못했습니다. 잠시 후 다시 확인하세요.", "No work request has been observed since registration. Check again shortly.", "注册后尚未观测到任务请求，请稍后检查。"],
  poll_stalled: ["작업 요청 지연", "Work polling delayed", "任务请求延迟"],
  poll_stalledHint: ["연결은 유지되지만 60초 넘게 작업 요청을 관측하지 못했습니다. 자동 업데이트 진행 여부, Runner 로그와 Coordinator 연결을 확인하세요.", "Heartbeat is current, but no work poll was observed for over 60 seconds. Check auto-update progress, Runner logs and the Coordinator connection.", "心跳正常，但超过 60 秒未观测到任务请求。请检查自动更新进度、Runner 日志和 Coordinator 连接。"],
  heartbeat_lost: ["연결 확인 지연", "Heartbeat overdue", "心跳超时"],
  heartbeat_lostHint: ["15초 넘게 heartbeat가 없습니다. Runner 서비스와 네트워크를 확인하세요. 이 정보만으로 종료 원인을 알 수는 없습니다.", "No heartbeat for over 15 seconds. Check the Runner service and network. This observation alone cannot identify the cause.", "超过 15 秒未收到心跳。请检查 Runner 服务和网络；仅凭此信息无法确定原因。"],
  stopped: ["종료 보고됨", "Stop reported", "已报告停止"],
  stoppedHint: ["Runner가 종료를 보고했습니다. 다시 사용하려면 서비스를 시작하고 새 heartbeat를 확인하세요.", "The Runner reported stopping. Start its service and check for a fresh heartbeat before using it again.", "Runner 已报告停止。如需再次使用，请启动服务并确认新心跳。"],
  busy: ["작업 실행 중", "Running work", "正在执行任务"],
  busyHint: ["배정된 파이프라인이 실행 중입니다. 실행 중에는 새 작업을 요청하지 않아도 정상입니다.", "An assigned pipeline is running. A Runner need not poll for new work while executing it.", "已分配的流水线正在运行。执行期间不请求新任务属于正常情况。"],
  draining: ["작업 마무리 중", "Finishing current work", "正在完成当前任务"],
  drainingHint: ["새 배정은 중지됐으며 기존 작업을 마무리하고 있습니다. 작업 완료 후 유지보수하세요.", "New assignments are paused while existing work finishes. Wait for completion before maintenance.", "已暂停新任务分配，正在完成现有任务。请等待完成后再维护。"],
  paused: ["유지보수 모드", "Maintenance mode", "维护模式"],
  pausedHint: ["새 작업 배정이 중지됐습니다. 점검 후 관리자가 배정을 재개할 수 있습니다.", "New assignments are paused. An administrator can resume them after maintenance.", "新任务分配已暂停。维护后管理员可以恢复分配。"],
  unknown: ["진단 정보 없음", "Diagnostics unavailable", "无诊断信息"],
  unknownHint: ["Coordinator가 진단 정보를 제공하지 않습니다.", "The Coordinator did not provide diagnostics.", "Coordinator 未提供诊断信息。"],
  missing: ["Runner 등록 정보를 찾을 수 없습니다.", "Runner registration is no longer available.", "找不到 Runner 注册信息。"],
  stale: ["갱신 실패 — 아래는 마지막으로 확인한 정보입니다.", "Refresh failed — showing the last observed data.", "刷新失败，以下显示最后观测到的信息。"],
  observed: ["관측 시각", "Observed at", "观测时间"],
  started: ["최근 등록", "Last registration", "最近注册"],
  heartbeat: ["마지막 연결 보고", "Last presence report", "最近连接报告"],
  poll: ["마지막 작업 요청", "Last work poll", "最近任务请求"],
  stop: ["종료 보고", "Stop reported at", "停止报告时间"],
  docker: ["Docker CLI (등록 시)", "Docker CLI (at registration)", "Docker CLI（注册时）"],
  none: ["관측 기록 없음", "Not observed", "尚无观测记录"],
};
function rdt(key) { return runnerDiagnosticCopy[key]?.[state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1] ?? key; }
function runnerDiagnosticCode(runner) {
  return ["ready", "starting", "poll_stalled", "heartbeat_lost", "stopped", "busy", "draining", "paused"].includes(runner.diagnostic) ? runner.diagnostic : "unknown";
}
function runnerDiagnosticTime(value) {
  if (!value) return rdt("none");
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date.toLocaleString(localeTag()) : rdt("none");
}
function renderRunnerDiagnostics() {
  if (!elements["runner-diagnostics-dialog"]?.open) return;
  const runner = state.runners.find(item => item.id === runnerDiagnosticId);
  elements["runner-diagnostics-title"].textContent = runner ? `${rdt("title")} · ${runner.name}` : rdt("title");
  elements["runner-diagnostics-freshness"].textContent = runnerLoadError ? rdt("stale") : "";
  elements["runner-diagnostics-facts"].replaceChildren();
  if (!runner) {
    elements["runner-diagnostics-status"].textContent = rdt("missing");
    elements["runner-diagnostics-hint"].textContent = "";
    return;
  }
  const code = runnerDiagnosticCode(runner);
  elements["runner-diagnostics-status"].textContent = rdt(code);
  elements["runner-diagnostics-hint"].textContent = rdt(`${code}Hint`);
  const facts = [
    ["Runner ID", runner.id], [t("Operating system"), `${runner.os} / ${runner.arch}`],
    [t("Version"), runner.version], [rdt("docker"), dockerStatusLabel(runner.docker_available)],
    [rdt("started"), runnerDiagnosticTime(runner.started_at)],
    [rdt("heartbeat"), runnerDiagnosticTime(runner.last_seen)],
    [rdt("poll"), runnerDiagnosticTime(runner.last_claim_at)],
    [rdt("stop"), runnerDiagnosticTime(runner.stopped_at)],
    [rdt("observed"), runnerDiagnosticTime(runner.observed_at)],
  ];
  for (const [label, value] of facts) {
    const term = document.createElement("dt"), detail = document.createElement("dd");
    term.textContent = label; detail.textContent = value;
    elements["runner-diagnostics-facts"].append(term, detail);
  }
}

function runnerMaintenanceLabel(runner) {
  return runner.draining ? t(runner.busy ? "runner.draining" : "runner.drained") : "";
}

function renderRunners() {
  const focused = document.activeElement?.closest("button[data-runner-action]");
  const focusId = focused?.dataset.runnerId, focusAction = focused?.dataset.runnerAction;
  const runners = state.runners.filter((runner) => !state.query || [runner.name, runner.id, runner.os, runner.arch, runner.version, runner.status, runnerMaintenanceLabel(runner), rdt(runnerDiagnosticCode(runner)), dockerStatusLabel(runner.docker_available)].some((value) => String(value).toLowerCase().includes(state.query)));
  elements["runner-table-body"].replaceChildren();
  document.querySelector(".runner-table-wrap").hidden = runners.length === 0;
  elements["runner-empty-state"].hidden = runners.length !== 0;
  for (const runner of runners) {
    const row = document.createElement("tr");
    const name = document.createElement("div");
    name.className = "runner-name";
    const details = document.createElement("button");
    details.type = "button"; details.className = "runner-title runner-details";
    details.textContent = runner.name;
    details.dataset.runnerAction = "diagnostics"; details.dataset.runnerId = runner.id;
    details.setAttribute("aria-label", `${rdt("title")}: ${runner.name}`);
    name.append(details, textNode(runner.id, "runner-id"));
    if (["heartbeat_lost", "poll_stalled", "stopped"].includes(runner.diagnostic)) {
      name.append(textNode(rdt(runner.diagnostic), "runner-diagnostic-note"));
    }
    const current = runner.current_pipeline_id ? runnerPipelineButton(runner.current_pipeline_id) : textNode(t(runner.busy ? "runner.restricted" : "dynamic.idle"), "runner-idle");
    const status = document.createElement("div");
    status.className = "runner-status";
    status.append(statusBadge(runner.status));
    if (runner.draining) status.append(textNode(runnerMaintenanceLabel(runner), "runner-maintenance"));
    const actions = document.createElement("div");
    actions.className = "runner-actions";
    const drain = document.createElement("button");
    drain.type = "button";
    drain.className = "button button--ghost runner-drain";
    drain.dataset.runnerAction = "drain";
    drain.dataset.runnerId = runner.id;
    drain.textContent = t(runner.draining ? "runner.resume" : "runner.pause");
    drain.title = t(runner.draining ? "runner.resumeHint" : "runner.pauseHint", { name: runner.name });
    drain.setAttribute("aria-label", `${drain.textContent}: ${runner.name}`);
    drain.disabled = runnerMutations.has(runner.id);
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "button button--danger runner-remove";
    remove.dataset.runnerAction = "remove";
    remove.dataset.runnerId = runner.id;
    remove.textContent = t("dynamic.remove");
    remove.disabled = runner.status !== "offline" || runner.busy || Boolean(runner.current_pipeline_id) || runnerMutations.has(runner.id);
    remove.title = remove.disabled ? t("dynamic.stopRunnerFirst") : t("dynamic.removeRunner", { name: runner.name });
    remove.setAttribute("aria-label", t("dynamic.removeRunner", { name: runner.name }));
    actions.append(drain, remove);
    row.append(
      cell(status),
      cell(name),
      cell(osBadge(runner.os)),
      cell(textNode(runner.arch, "runner-meta")),
      cell(textNode(`v${runner.version}`, "runner-meta")),
      cell(dockerBadge(runner.docker_available)),
      cell(textNode(relativeTime(runner.last_seen), "time-cell")),
      cell(current),
      cell(state.user?.role === "admin" ? actions : textNode("—", "runner-meta")),
    );
    elements["runner-table-body"].append(row);
  }
  const total = state.runners.length;
  const online = state.runners.filter((runner) => runner.status === "online").length;
  elements["runner-stat-total"].textContent = String(total);
  elements["runner-stat-online"].textContent = String(online);
  elements["runner-stat-offline"].textContent = String(total - online);
  elements["runner-count"].textContent = runners.length === total
    ? tc("dynamic.countRunners", total)
    : t("dynamic.countFilteredRunners", { shown: runners.length, total });
  elements["nav-runner-count"].textContent = `${online}/${total}`;
  elements["nav-runner-count"].hidden = total === 0;
  if (focusId) [...elements["runner-table-body"].querySelectorAll("button[data-runner-action]")]
    .find(button => button.dataset.runnerId === focusId && button.dataset.runnerAction === focusAction)?.focus({ preventScroll: true });
  renderRunnerDiagnostics();
}

function dockerStatusLabel(available) {
  return t(available === true ? "docker.installed" : available === false ? "docker.notInstalled" : "docker.unknown");
}

function dockerBadge(available) {
  const badge = document.createElement("span");
  const state = available === true ? "installed" : available === false ? "missing" : "unknown";
  badge.className = `docker-badge docker-badge--${state}`;
  badge.textContent = dockerStatusLabel(available);
  return badge;
}

function runnerAction(event) {
  const button = event.target.closest("button[data-runner-action]");
  if (!button || button.disabled || !state.user) return;
  const runner = state.runners.find((item) => item.id === button.dataset.runnerId);
  if (!runner) return;
  if (button.dataset.runnerAction === "diagnostics") {
    runnerDiagnosticId = runner.id;
    elements["runner-diagnostics-dialog"].showModal();
    renderRunnerDiagnostics();
    return;
  }
  if (state.user.role !== "admin") return;
  if (button.dataset.runnerAction === "drain") {
    void setRunnerDraining(runner.id, !runner.draining);
    return;
  }
  if (button.dataset.runnerAction !== "remove") return;
  state.runnerToRemove = runner;
  elements["remove-runner-name"].textContent = runner.name;
  elements["remove-runner-confirmation"].value = "";
  elements["remove-runner-confirmation"].setCustomValidity("");
  elements["remove-runner-dialog"].showModal();
  window.setTimeout(() => elements["remove-runner-confirmation"].focus(), 0);
}

async function setRunnerDraining(id, draining) {
  if (state.user?.role !== "admin" || runnerMutations.has(id)) return;
  runnerMutations.add(id);
  ++runnerListRequest; // Invalidate polls started before the mutation.
  renderRunners();
  try {
    await api(`/api/v1/runners/${encodeURIComponent(id)}/drain`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ draining }),
    });
    const runner = state.runners.find(item => item.id === id);
    if (runner) runner.draining = draining;
    toast(t(draining ? "runner.paused" : "runner.resumed"), "success");
  } catch (error) { toast(error.message, "error"); }
  finally {
    runnerMutations.delete(id);
    await loadRunners(false);
    renderRunners();
  }
}

async function removeRunner(event) {
  event.preventDefault();
  const runner = state.runnerToRemove;
  if (!runner || elements["remove-runner-confirmation"].value !== runner.name) {
    elements["remove-runner-confirmation"].setCustomValidity(t("dynamic.runnerNameMismatch"));
    elements["remove-runner-confirmation"].reportValidity();
    return;
  }
  elements["remove-runner-confirmation"].setCustomValidity("");
  elements["confirm-remove-runner-button"].disabled = true;
  try {
    await api(`/api/v1/runners/${encodeURIComponent(runner.id)}`, {
      method: "DELETE",
      headers: { "X-CSRF-Token": csrfToken() },
    });
    elements["remove-runner-dialog"].close();
    state.runnerToRemove = null;
    toast(t("dynamic.runnerRemoved"), "success");
    await loadRunners(false);
  } catch (error) { toast(error.message, "error"); }
  finally { elements["confirm-remove-runner-button"].disabled = false; }
}

function osBadge(os) {
  const badge = document.createElement("span");
  badge.className = `os-badge os-badge--${os}`;
  badge.textContent = ({ windows: "Windows", linux: "Linux", macos: "macOS" })[os] || os;
  return badge;
}

function viewBadge(name) {
  const badge = document.createElement("span");
  badge.className = "view-badge";
  badge.textContent = `${t("Sparse View")} · ${name}`;
  return badge;
}

function runnerPipelineButton(id) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "runner-pipeline";
  button.textContent = `#${id.slice(0, 8)}`;
  button.addEventListener("click", () => void openPipeline(id));
  return button;
}

function refreshSection(notify) {
  cancelPipelineHistorySearch();
  if (state.section === "overview") return Promise.all([loadOverview(), loadRepositories(false), loadPipelines(notify)]);
  if (isManagement()) return discardManagement() ? loadManagement() : Promise.resolve();
  if (state.section === "repositories") return loadRepositories(notify);
  if (state.section === "ci-settings") return discardRepositoryConfigEdit() ? loadCiSettings(notify) : Promise.resolve();
  if (state.section === "repository-links") return loadRepositoryLinksPage(state.repositoryLinksName, notify);
  if (state.section === "repository-tree") return loadRepositoryTreePage();
  if (state.section === "runners") return loadRunners(notify);
  if (state.section === "graphs") return loadPipelines(false).then(() => loadPipelineGraphs(notify));
  return loadPipelines(notify);
}

function renderRepositories() {
  const repositories = state.repositories.filter((repository) => !state.query || [repository.name, repository.id, repository.url].some((value) => String(value).toLowerCase().includes(state.query)));
  elements["repository-list"].replaceChildren();
  elements["repository-list"].hidden = repositories.length === 0;
  elements["repository-empty-state"].hidden = repositories.length !== 0;
  for (const repository of repositories) {
    const card = document.createElement("article");
    card.className = "repository-card";
    card.dataset.name = repository.name;
    const icon = document.createElement("span");
    icon.className = "repository-icon";
    icon.textContent = repository.name.slice(0, 2).toUpperCase();
    const content = document.createElement("div");
    content.className = "repository-card-content";
    const heading = document.createElement("div");
    heading.className = "repository-card-heading";
    const name = document.createElement("a"); name.className = "repository-open"; name.href = repositorySectionHash("repository-tree", repository.url); name.textContent = repository.name;
    const id = document.createElement("span"); id.textContent = `ID ${repository.id.slice(0, 12)}`;
    heading.append(name);
    const meta = document.createElement("div");
    meta.className = "repository-card-meta";
    const storageBackend = repository.storage_backend === "local_file" ? "local_file" : "dynamodb_s3";
    const storageBadge = document.createElement("span");
    storageBadge.className = `repository-storage-badge repository-storage-badge--${storageBackend}`;
    storageBadge.textContent = t(storageBackend === "local_file" ? "Local File" : "DynamoDB + S3");
    storageBadge.title = `${t("Storage Backend")}: ${storageBadge.textContent}`;
    storageBadge.setAttribute("aria-label", `${t("Storage Backend")}: ${storageBadge.textContent}`);
    meta.append(id, storageBadge);
    const url = document.createElement("code"); url.textContent = repository.url;
    content.append(heading, meta, url);
    const actions = document.createElement("div"); actions.className = "repository-actions";
    actions.append(repositoryButton("folders", t("Folder view")), repositoryButton("copy", t("dynamic.copyUrl")), repositoryButton("pipeline", t("dynamic.runPipeline")), repositoryButton("branches", t("dynamic.pipelineBranches")), repositoryButton("delete", t("dynamic.delete"), "button--danger"));
    card.append(icon, content, actions);
    elements["repository-list"].append(card);
  }
  const total = state.repositories.length;
  const label = repositories.length === total
    ? tc("dynamic.countRepositories", total)
    : t("dynamic.countFilteredRepositories", { shown: repositories.length, total });
  elements["repository-count"].textContent = label;
  elements["nav-repository-count"].textContent = String(total);
  elements["nav-repository-count"].hidden = total === 0;
}

function repositoryButton(action, label, extraClass = "") {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.action = action;
  button.className = `button button--ghost repository-action ${extraClass}`.trim();
  button.textContent = label;
  return button;
}

async function repositoryAction(event) {
  const button = event.target.closest("button[data-action]");
  const card = event.target.closest("[data-name]");
  if (!button || !card) return;
  const repository = state.repositories.find((item) => item.name === card.dataset.name);
  if (!repository) return;
  if (button.dataset.action === "folders") {
    navigateRepositorySection("repository-tree", repository.url);
  } else if (["history", "graphs"].includes(button.dataset.action)) {
    navigateRepositorySection(button.dataset.action === "history" ? "pipelines" : "graphs", repository.url);
  } else if (button.dataset.action === "copy") {
    try { await navigator.clipboard.writeText(repository.url); toast(t("dynamic.urlCopied"), "success"); }
    catch (_) { toast(t("dynamic.copyFailed"), "error"); }
  } else if (button.dataset.action === "config") {
    void openRepositoryConfig(repository);
  } else if (button.dataset.action === "links") {
    openRepositoryLinks(repository);
  } else if (button.dataset.action === "pipeline") {
    openNewPipeline(repository.url);
  } else if (button.dataset.action === "branches") {
    void openRepositoryPipelineBranches(repository);
  } else if (button.dataset.action === "delete") {
    state.repositoryToDelete = repository.name;
    elements["delete-repository-name"].textContent = repository.name;
    elements["delete-repository-confirmation"].value = "";
    elements["delete-repository-dialog"].showModal();
    window.setTimeout(() => elements["delete-repository-confirmation"].focus(), 0);
  }
}

async function openRepositoryPipelineBranches(repository) {
  state.repositoryBranchesName = repository.name;
  elements["repository-branches-name"].textContent = repository.name;
  elements["repository-branches-list"].replaceChildren(textNode(t("dynamic.loadingBranches"), "management-note"));
  elements["save-repository-branches-button"].disabled = true;
  elements["repository-branches-dialog"].showModal();
  try {
    const settings = await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/pipeline-branches`);
    if (state.repositoryBranchesName !== repository.name) return;
    elements["repository-branches-list"].replaceChildren();
    if (!settings.branches.length) {
      elements["repository-branches-list"].append(textNode(t("dynamic.noRemoteBranches"), "management-note"));
    }
    for (const branch of settings.branches) {
      const label = document.createElement("label"); label.className = "repository-branch-option";
      const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.value = branch.name; checkbox.checked = settings.selected.includes(branch.name);
      const name = document.createElement("strong"); name.textContent = branch.name;
      label.append(checkbox, name); elements["repository-branches-list"].append(label);
    }
    elements["save-repository-branches-button"].disabled = false;
  } catch (error) {
    toast(error.message, "error");
  }
}

async function saveRepositoryPipelineBranches(event) {
  event.preventDefault();
  const name = state.repositoryBranchesName;
  if (!name) return;
  const button = elements["save-repository-branches-button"];
  button.disabled = true;
  try {
    const branches = [...elements["repository-branches-list"].querySelectorAll('input[type="checkbox"]:checked')].map(input => input.value);
    await api(`/api/v1/repositories/${encodeURIComponent(name)}/pipeline-branches`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ branches }),
    });
    elements["repository-branches-dialog"].close();
    state.repositoryBranchesName = null;
    toast(t("dynamic.branchPolicySaved"), "success");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
}

function openRepositoryLinks(repository) {
  navigateRepositorySection("repository-links", repository.url);
}

async function loadRepositoryLinksPage(preferredName, notify = false) {
  const saved = repositoryLinkPreference();
  preferredName ||= saved?.name;
  const request = ++state.repositoryLinksRequest;
  state.repositoryLinksStatus = "loading";
  state.repositoryLinksError = "";
  state.repositoryLinkOperations = [];
  state.repositoryLinkOperationsError = "";
  renderRepositoryLinks();
  await loadRepositories(false);
  if (state.section !== "repository-links" || request !== state.repositoryLinksRequest) return;

  const candidates = await prioritizeRepositoryLinkRepositories();
  if (state.section !== "repository-links" || request !== state.repositoryLinksRequest) return;

  const repositorySelect = elements["repository-link-repository"];
  const branchSelect = elements["repository-link-branch"];
  repositorySelect.replaceChildren();
  branchSelect.replaceChildren();
  for (const candidate of candidates) repositorySelect.add(new Option(candidate.repository.name, candidate.repository.name));
  const candidate = candidates.find(item => item.repository.name === preferredName) ?? candidates[0];
  if (!candidate) {
    state.repositoryLinksName = null;
    state.repositoryLinksBranch = null;
    state.repositoryLinksRevision = null;
    state.repositoryLinks = [];
    state.repositoryLinksStatus = "no-repositories";
    renderRepositoryLinks();
    return;
  }

  const repository = candidate.repository;
  state.repositoryLinksName = repository.name;
  repositorySelect.value = repository.name;
  try {
    const branches = candidate.branches ?? await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/branches`);
    if (request !== state.repositoryLinksRequest || state.repositoryLinksName !== repository.name) return;
    for (const branch of branches) branchSelect.add(new Option(branch.name, branch.name));
    if (!branches.length) {
      state.repositoryLinksBranch = null;
      state.repositoryLinksRevision = null;
      state.repositoryLinks = [];
      state.repositoryLinksStatus = "no-branches";
      renderRepositoryLinks();
      return;
    }
    const preferredBranch = selectRepositoryBranch(branches, repository.url).name;
    branchSelect.value = preferredBranch;
    state.repositoryLinksBranch = preferredBranch;
    if (candidate.links && candidate.branch === preferredBranch) {
      applyRepositoryLinks(candidate.links);
    } else {
      await loadRepositoryLinks();
    }
    if (notify && state.repositoryLinksStatus === "ready") toast(t("dynamic.refreshRepositoryLinks"), "success");
  } catch (error) {
    if (request !== state.repositoryLinksRequest) return;
    state.repositoryLinksStatus = "error";
    state.repositoryLinksError = error.message;
    renderRepositoryLinks();
    toast(error.message, "error");
  }
}

function repositoryLinkPreference(value) {
  const key = `lorehub_links_${state.user?.id ?? "anonymous"}`;
  try {
    if (value) window.localStorage.setItem(key, JSON.stringify(value));
    return JSON.parse(window.localStorage.getItem(key) || "null");
  } catch (_) { return null; }
}

async function prioritizeRepositoryLinkRepositories() {
  let summary = [];
  try { summary = await api("/api/v1/repository-links/summary"); } catch (_) { /* Keep the page usable if the index is unavailable. */ }
  return scopedRepositories().map((repository, index) => {
    const branches = summary.filter(row => repositoryIdentifier(row.resource_id) === repositoryIdentifier(repository.id) && row.count > 0);
    return { repository, index, hasLinks: branches.length > 0, branch: (branches.find(row => row.branch === "main") ?? branches[0])?.branch };
  }).sort((left, right) => Number(right.hasLinks) - Number(left.hasLinks) || left.index - right.index);
}
function applyRepositoryLinks(result) {
  if (state.repositoryLinksRevision !== result.revision) {
    state.repositoryLinkOpenPaths.clear();
    state.repositoryLinkContent.clear();
    state.repositoryLinkCollapsedPaths.delete(repositoryLinkScopePath(""));
  }
  state.repositoryLinksRevision = result.revision;
  state.repositoryLinks = result.links ?? [];
  state.repositoryLinksStatus = "ready";
  repositoryLinkPreference({ name: state.repositoryLinksName, branch: state.repositoryLinksBranch });
  state.updatedAt.repositoryLinks = new Date();
  renderRepositoryLinks();
  renderUpdatedLabels();
}

async function loadRepositoryLinks() {
  const name = state.repositoryLinksName;
  const branch = elements["repository-link-branch"].value;
  if (!name || !branch) return;
  rememberRepositoryBranch(branch);
  state.repositoryLinksBranch = branch;
  state.repositoryLinksStatus = "loading";
  state.repositoryLinksError = "";
  state.repositoryLinksRevision = null;
  state.repositoryLinks = [];
  state.repositoryLinkOperations = [];
  state.repositoryLinkOperationsError = "";
  const request = ++state.repositoryLinksRequest;
  renderRepositoryLinks();
  try {
    const [result] = await Promise.all([
      api(`/api/v1/repositories/${encodeURIComponent(name)}/links?branch=${encodeURIComponent(branch)}`),
      loadRepositoryLinkOperations(name, branch),
    ]);
    if (request !== state.repositoryLinksRequest || state.repositoryLinksName !== name || elements["repository-link-branch"].value !== branch) return;
    applyRepositoryLinks(result);
  } catch (error) {
    if (request !== state.repositoryLinksRequest) return;
    state.repositoryLinksStatus = "error";
    state.repositoryLinksError = error.message;
    renderRepositoryLinks();
    toast(error.message, "error");
  }
}

function repositoryIdentifier(value) {
  return String(value ?? "").replace(/^urc-/i, "").toLowerCase();
}

function repositoryLinkTree(links) {
  const root = { children: new Map() };
  for (const link of links) {
    let parent = root;
    let path = "";
    for (const name of link.path.split("/").filter(Boolean)) {
      path = path ? `${path}/${name}` : name;
      if (!parent.children.has(name)) parent.children.set(name, { name, path, children: new Map(), link: null });
      parent = parent.children.get(name);
    }
    parent.link = link;
  }
  return root;
}

function repositoryLinkScopePath(path) {
  return `${state.repositoryLinksName}\0${state.repositoryLinksBranch}\0${path}`;
}
function repositoryLinkContainingPath(path, includeSelf = false) {
  return state.repositoryLinks.find(link => (includeSelf && path === link.path) || path.startsWith(`${link.path}/`));
}

function toggleRepositoryLinkFolder(path) {
  const key = repositoryLinkScopePath(path);
  if (state.repositoryLinkCollapsedPaths.has(key)) state.repositoryLinkCollapsedPaths.delete(key);
  else state.repositoryLinkCollapsedPaths.add(key);
  renderRepositoryLinks();
}

async function loadRepositoryLinkContent(path) {
  const name = state.repositoryLinksName, branch = state.repositoryLinksBranch, revision = state.repositoryLinksRevision;
  const key = repositoryLinkScopePath(path);
  const pending = { entries: [], loading: true, error: "" };
  state.repositoryLinkContent.set(key, pending);
  renderRepositoryLinks();
  try {
    const params = new URLSearchParams({ revision, path });
    const entries = await api(`/api/v1/repositories/${encodeURIComponent(name)}/tree?${params}`);
    if (name === state.repositoryLinksName && branch === state.repositoryLinksBranch && revision === state.repositoryLinksRevision && state.repositoryLinkContent.get(key) === pending) {
      state.repositoryLinkContent.set(key, { entries, loading: false, error: "" });
      renderRepositoryLinks();
    }
  } catch (error) {
    if (name === state.repositoryLinksName && branch === state.repositoryLinksBranch && revision === state.repositoryLinksRevision && state.repositoryLinkContent.get(key) === pending) {
      state.repositoryLinkContent.set(key, { entries: [], loading: false, error: error.message });
      renderRepositoryLinks();
    }
  }
}

function toggleRepositoryLinkContent(path) {
  const key = repositoryLinkScopePath(path);
  if (state.repositoryLinkOpenPaths.has(key)) {
    state.repositoryLinkOpenPaths.delete(key);
    renderRepositoryLinks();
  } else {
    state.repositoryLinkOpenPaths.add(key);
    if (state.repositoryLinkContent.has(key)) renderRepositoryLinks();
    else void loadRepositoryLinkContent(path);
  }
}

function toggleRepositoryLinkDetails(path) {
  const key = repositoryLinkScopePath(path);
  state.repositoryLinkSelectedPath = state.repositoryLinkSelectedPath === key ? null : key;
  renderRepositoryLinks();
}

function renderRepositoryLinkContent(path, excludedNames = new Set(), nested = false) {
  const content = state.repositoryLinkContent.get(repositoryLinkScopePath(path));
  const container = document.createElement("div"); container.className = nested ? "repository-tree-children" : "repository-link-content";
  container.setAttribute("aria-busy", String(Boolean(content?.loading)));
  if (!content || content.loading || content.error) {
    const message = textNode(content?.error || t("Loading folder…"), "repository-tree-message");
    if (content?.error) {
      message.setAttribute("role", "alert");
      const retry = document.createElement("button"); retry.type = "button"; retry.className = "button button--ghost"; retry.textContent = t("Retry");
      retry.addEventListener("click", () => void loadRepositoryLinkContent(path)); message.append(retry);
    }
    container.append(message);
    return container;
  }
  const entries = content.entries.filter(entry => !excludedNames.has(entry.name));
  if (!entries.length) {
    if (!content.entries.length) container.append(textNode(t("No files or folders"), "repository-tree-message"));
    return container;
  }
  const list = document.createElement("ul"); list.className = "repository-tree-nodes";
  for (const entry of entries) {
    const childPath = `${path}/${entry.name}`;
    const item = document.createElement("li"); item.className = "repository-tree-item";
    if (entry.kind === "directory") {
      const open = state.repositoryLinkOpenPaths.has(repositoryLinkScopePath(childPath));
      const button = document.createElement("button"); button.type = "button"; button.className = "repository-tree-toggle";
      button.dataset.nodePath = childPath; button.dataset.nodeKind = "directory";
      button.dataset.focusKey = `content:${childPath}`;
      button.setAttribute("aria-expanded", String(open));
      button.append(textNode(open ? "▾" : "▸", "repository-tree-icon"), textNode(entry.name, "repository-tree-name"));
      if (entry.is_link) button.append(textNode("LINK", "repository-tree-link"));
      button.addEventListener("click", () => toggleRepositoryLinkContent(childPath)); item.append(button);
      if (open) item.append(renderRepositoryLinkContent(childPath, new Set(), true));
    } else {
      const row = document.createElement("div"); row.className = "repository-tree-file";
      row.dataset.nodePath = childPath; row.dataset.nodeKind = "file";
      row.append(textNode("·", "repository-tree-icon"), textNode(entry.name, "repository-tree-name")); item.append(row);
    }
    list.append(item);
  }
  container.append(list);
  return container;
}

function renderRepositoryLinkDetail(link) {
  const source = state.repositories.find(repository => repositoryIdentifier(repository.id) === repositoryIdentifier(link.source_repository_id));
  const card = document.createElement("article"); card.className = "repository-link-card"; card.dataset.path = link.path;
  const identity = document.createElement("div"); identity.className = "repository-link-identity";
  identity.append(textNode(`${source?.name ?? t("Source unavailable")} / ${link.source_branch_name ?? "?"} / ${link.source_path}`, "repository-link-path"));
  identity.append(textNode(`→ ${state.repositoryLinksName} / ${state.repositoryLinksBranch} / ${link.path}`, "repository-link-target"));
  const lastSync = document.createElement("div"); lastSync.className = "repository-link-detail";
  lastSync.append(textNode(t("Last successful sync"), "repository-link-detail-label"), textNode(link.last_success_at ? new Date(link.last_success_at).toLocaleString(state.locale) : t("No sync recorded"), "repository-link-detail-value"));
  const policy = document.createElement("div"); policy.className = "repository-link-detail";
  policy.append(textNode(t("Sync policy"), "repository-link-detail-label"), textNode(t(link.auto_update ? "Automatic sync" : "Manual sync"), "repository-link-detail-value"));
  const details = document.createElement("details"); details.className = "repository-link-technical";
  const summary = document.createElement("summary"); summary.textContent = t("Technical details"); details.append(summary);
  for (const [label, value] of [["Source repository", link.source_repository_id], ["Source branch", link.source_branch_id], ["dynamic.pinnedRevision", link.source_revision], ["Latest Source revision", link.latest_revision ?? "—"], ["Lore", t(link.tracking ? "dynamic.tracking" : "dynamic.fixed")]]) {
    details.append(textNode(`${t(label)}: ${value}`, "repository-link-source"));
  }
  card.append(identity, lastSync, policy, details);
  if (link.last_error) card.append(textNode(link.last_error, "repository-link-inline-error"));
  return card;
}

function renderRepositoryLinkNodes(nodes) {
  const list = document.createElement("ul"); list.className = "repository-tree-nodes repository-link-tree";
  const sorted = [...nodes.values()].sort((left, right) => left.name.localeCompare(right.name, state.locale));
  for (const node of sorted) {
    const item = document.createElement("li"); item.className = "repository-tree-item";
    const row = document.createElement("div"); row.className = "repository-link-row";
    row.dataset.nodePath = node.path; row.dataset.nodeKind = node.link ? "link" : "directory";
    const hasChildren = node.children.size > 0 || Boolean(node.link);
    const collapsed = node.link ? !state.repositoryLinkOpenPaths.has(repositoryLinkScopePath(node.path)) : state.repositoryLinkCollapsedPaths.has(repositoryLinkScopePath(node.path));
    if (hasChildren) {
      const expand = document.createElement("button"); expand.type = "button"; expand.className = "repository-link-expand";
      expand.dataset.focusKey = `folder:${node.path}`;
      expand.setAttribute("aria-label", `${t(collapsed ? "Expand folder" : "Collapse folder")}: ${node.path}`);
      expand.setAttribute("aria-expanded", String(!collapsed));
      expand.textContent = collapsed ? "▸" : "▾";
      expand.addEventListener("click", () => node.link ? toggleRepositoryLinkContent(node.path) : toggleRepositoryLinkFolder(node.path));
      row.append(expand);
    } else row.append(textNode("·", "repository-link-spacer"));
    const name = document.createElement("button"); name.type = "button"; name.className = "repository-link-name";
    name.textContent = node.name; name.title = node.path; name.dataset.focusKey = `name:${node.path}`;
    if (node.link) {
      const selected = state.repositoryLinkSelectedPath === repositoryLinkScopePath(node.path);
      name.setAttribute("aria-expanded", String(selected));
      name.setAttribute("aria-label", `${t("Link details")}: ${node.path}`);
      name.addEventListener("click", () => toggleRepositoryLinkDetails(node.path));
    } else name.addEventListener("click", () => toggleRepositoryLinkFolder(node.path));
    row.append(name);
    if (node.link) {
      const status = ["current", "outdated", "failed", "unknown"].includes(node.link.status) ? node.link.status : "unknown";
      const labels = { current: "Up to date", outdated: "Update available", failed: "Sync failed", unknown: "Source unavailable" };
      row.append(textNode("LINK", "repository-tree-link"));
      row.append(textNode(t(labels[status]), `repository-link-badge repository-link-badge--${status}`));
    }
    item.append(row);
    if (node.link && state.repositoryLinkSelectedPath === repositoryLinkScopePath(node.path)) item.append(renderRepositoryLinkDetail(node.link));
    if (hasChildren && !collapsed) {
      const children = document.createElement("div"); children.className = "repository-tree-children";
      if (node.children.size) children.append(renderRepositoryLinkNodes(node.children));
      if (node.link) children.append(renderRepositoryLinkContent(node.path, new Set(node.children.keys())));
      item.append(children);
    }
    list.append(item);
  }
  return list;
}

function renderRepositoryLinkRoot(nodes) {
  const list = document.createElement("ul"); list.className = "repository-tree-nodes repository-link-tree";
  list.setAttribute("aria-label", t("Linked paths"));
  const item = document.createElement("li"); item.className = "repository-tree-item";
  const row = document.createElement("div"); row.className = "repository-link-row repository-link-root";
  row.dataset.nodePath = ""; row.dataset.nodeKind = "root";
  const collapsed = state.repositoryLinkCollapsedPaths.has(repositoryLinkScopePath(""));
  const label = `${state.repositoryLinksName} / ${state.repositoryLinksBranch}`;
  const expand = document.createElement("button"); expand.type = "button"; expand.className = "repository-link-expand";
  expand.dataset.focusKey = "root-folder";
  expand.setAttribute("aria-label", `${t(collapsed ? "Expand folder" : "Collapse folder")}: ${label}`);
  expand.setAttribute("aria-expanded", String(!collapsed));
  expand.textContent = collapsed ? "▸" : "▾";
  expand.addEventListener("click", () => toggleRepositoryLinkFolder(""));
  const name = document.createElement("button"); name.type = "button"; name.className = "repository-link-name repository-link-root-name";
  name.textContent = label; name.dataset.focusKey = "root-name"; name.setAttribute("aria-expanded", String(!collapsed));
  name.addEventListener("click", () => toggleRepositoryLinkFolder(""));
  row.append(expand, name, textNode("ROOT", "repository-link-root-badge"));
  item.append(row);
  if (!collapsed) {
    const children = document.createElement("div"); children.className = "repository-tree-children";
    if (nodes.size) children.append(renderRepositoryLinkNodes(nodes));
    else {
      const empty = document.createElement("div"); empty.className = "repository-link-empty";
      empty.append(textNode(t("Repository link가 없습니다")), textNode(t("다른 저장소의 경로를 이 branch에 연결하세요.")));
      children.append(empty);
    }
    item.append(children);
  }
  list.append(item);
  return list;
}

let repositoryLinkContextMenu = null;
let repositoryLinkContextTarget = null;

function closeRepositoryLinkContextMenu(restoreFocus = false) {
  repositoryLinkContextMenu?.remove();
  repositoryLinkContextMenu = null;
  if (restoreFocus) repositoryLinkContextTarget?.focus({ preventScroll: true });
  repositoryLinkContextTarget = null;
}

function openRepositoryLinkContextMenu(event) {
  const node = event.target.closest("[data-node-path]");
  if (!node || state.repositoryLinksStatus !== "ready") return;
  event.preventDefault();
  closeRepositoryLinkContextMenu();
  const path = node.dataset.nodePath;
  const kind = node.dataset.nodeKind;
  const targetFolder = kind === "file" ? path.slice(0, Math.max(0, path.lastIndexOf("/"))) : path;
  const link = kind === "link" ? state.repositoryLinks.find(item => item.path === path) : null;
  const parentLink = repositoryLinkContainingPath(targetFolder, true);
  const menu = document.createElement("div"); menu.className = "repository-link-context-menu"; menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", path || state.repositoryLinksName);
  const add = document.createElement("button"); add.type = "button"; add.setAttribute("role", "menuitem");
  add.textContent = t("Add link"); add.disabled = state.repositoryLinksBusy || state.repositories.length < 2 || Boolean(parentLink);
  if (parentLink) add.title = t("A LINK cannot be created below {path} because it is already a LINK.", { path: parentLink.path });
  add.addEventListener("click", () => { closeRepositoryLinkContextMenu(); openNewRepositoryLink(targetFolder); });
  const actions = [add];
  if (link) {
    for (const [action, label] of [["update", t("dynamic.updateLink")], ["policy", t(link.auto_update ? "Switch to manual sync" : "Enable automatic sync")]]) {
      const button = document.createElement("button"); button.type = "button"; button.setAttribute("role", "menuitem");
      button.textContent = label; button.disabled = state.repositoryLinksBusy;
      button.addEventListener("click", () => { closeRepositoryLinkContextMenu(); void performRepositoryLinkAction(action, path); });
      actions.push(button);
    }
  }
  const remove = document.createElement("button"); remove.type = "button"; remove.setAttribute("role", "menuitem");
  remove.textContent = t("Delete link"); remove.disabled = state.repositoryLinksBusy || !link;
  remove.addEventListener("click", () => { closeRepositoryLinkContextMenu(); void performRepositoryLinkAction("remove", path); });
  if (kind !== "root") {
    actions.push(remove);
  }
  menu.append(...actions);
  if (parentLink) menu.append(textNode(add.title, "repository-link-context-note"));
  if (kind !== "root" && !link) menu.append(textNode(t("Only linked folders can be deleted here."), "repository-link-context-note"));
  document.body.append(menu);
  const rect = menu.getBoundingClientRect();
  const anchor = node.getBoundingClientRect();
  const x = event.clientX || anchor.left, y = event.clientY || anchor.bottom;
  menu.style.left = `${Math.max(8, Math.min(x, window.innerWidth - rect.width - 8))}px`;
  menu.style.top = `${Math.max(8, Math.min(y, window.innerHeight - rect.height - 8))}px`;
  repositoryLinkContextMenu = menu;
  repositoryLinkContextTarget = node;
  actions.find(button => !button.disabled)?.focus();
}

function renderRepositoryLinks() {
  closeRepositoryLinkContextMenu();
  renderRepositoryLinkOperations();
  const ready = state.repositoryLinksStatus === "ready";
  const list = elements["repository-link-list"];
  const focused = list.contains(document.activeElement) ? document.activeElement.dataset.focusKey : null;
  elements["repository-link-repository"].disabled = state.repositoryLinksBusy || state.repositoryLinksStatus === "loading" || state.repositoryLinksStatus === "no-repositories";
  elements["repository-link-branch"].disabled = state.repositoryLinksBusy || state.repositoryLinksStatus === "loading" || state.repositoryLinksStatus === "no-branches" || state.repositoryLinksStatus === "no-repositories";
  elements["repository-link-revision"].textContent = state.repositoryLinksRevision ?? "—";
  elements["repository-link-revision"].title = state.repositoryLinksRevision ?? "";
  elements["repository-link-count"].textContent = tc("dynamic.countLinks", ready ? state.repositoryLinks.length : 0);
  list.replaceChildren();

  if (!ready) {
    const message = state.repositoryLinksStatus === "no-repositories" ? t("dynamic.noRepositories")
      : state.repositoryLinksStatus === "no-branches" ? t("dynamic.noRemoteBranches")
      : state.repositoryLinksStatus === "error" ? state.repositoryLinksError
      : t("dynamic.loadingLinks");
    list.append(textNode(message, "repository-link-message"));
    return;
  }

  list.append(renderRepositoryLinkRoot(repositoryLinkTree(state.repositoryLinks).children));
  if (focused) [...list.querySelectorAll("[data-focus-key]")].find(control => control.dataset.focusKey === focused)?.focus({ preventScroll: true });
}

function openNewRepositoryLink(folderPath = "") {
  if (state.repositoryLinksBusy || state.repositoryLinksStatus !== "ready" || !state.repositoryLinksRevision) return;
  if (repositoryLinkContainingPath(folderPath, true)) return;
  const sources = state.repositories.filter(repository => repository.name !== state.repositoryLinksName);
  elements["repository-link-form"].reset();
  elements["repository-link-path"].value = folderPath ? `${folderPath}/` : "";
  state.repositoryLinkDraft = null;
  setRepositoryLinkFormError();
  elements["repository-link-progress"].hidden = true;
  elements["repository-link-root-name"].textContent = `${state.repositoryLinksName} / ${state.repositoryLinksBranch}`;
  elements["repository-link-source-repository"].replaceChildren();
  for (const repository of sources) elements["repository-link-source-repository"].add(new Option(repository.name, repository.name));
  elements["repository-link-source-path"].value = folderPath || ".";
  elements["create-repository-link-button"].disabled = true;
  if (!sources.length) elements["repository-link-source-repository"].add(new Option(t("dynamic.noSourceRepositories"), ""));
  elements["new-repository-link-dialog"].showModal();
  void loadRepositoryLinkSourceBranches();
  renderRepositoryLinkPreview();
  window.setTimeout(() => elements["repository-link-path"].focus(), 0);
}

async function loadRepositoryLinkSourceBranches() {
  const repository = elements["repository-link-source-repository"].value;
  const select = elements["repository-link-source-branch"];
  select.replaceChildren();
  select.disabled = true;
  elements["create-repository-link-button"].disabled = true;
  if (!repository) return;
  try {
    const branches = await api(`/api/v1/repositories/${encodeURIComponent(repository)}/branches`);
    if (elements["repository-link-source-repository"].value !== repository) return;
    for (const branch of branches) select.add(new Option(branch.name, branch.name));
    if (branches.length) {
      select.value = branches.some(branch => branch.name === "main") ? "main" : branches[0].name;
      select.disabled = false;
    }
    elements["create-repository-link-button"].disabled = branches.length === 0;
    renderRepositoryLinkPreview();
  } catch (error) {
    toast(error.message, "error");
  }
}

function renderRepositoryLinkPreview() {
  const source = elements["repository-link-source-repository"].value;
  const branch = elements["repository-link-source-branch"].value;
  elements["repository-link-preview"].textContent = `${source} / ${branch || "?"} / ${elements["repository-link-source-path"].value || "."} → ${state.repositoryLinksName} / ${state.repositoryLinksBranch} / ${elements["repository-link-path"].value || "…"}`;
}

async function loadRepositoryLinkOperations(name = state.repositoryLinksName, branch = state.repositoryLinksBranch) {
  if (!name || !branch) return;
  const request = ++state.repositoryLinkOperationsRequest;
  try {
    const records = await api(`/api/v1/repositories/${encodeURIComponent(name)}/link-operations?branch=${encodeURIComponent(branch)}`);
    if (request !== state.repositoryLinkOperationsRequest || state.repositoryLinksName !== name || state.repositoryLinksBranch !== branch) return;
    state.repositoryLinkOperations = records;
    state.repositoryLinkOperationsError = "";
  } catch (error) {
    if (request !== state.repositoryLinkOperationsRequest || state.repositoryLinksName !== name || state.repositoryLinksBranch !== branch) return;
    state.repositoryLinkOperationsError = error.message;
  }
  renderRepositoryLinkOperations();
}

function renderRepositoryLinkOperations() {
  const list = elements["repository-link-operations"];
  if (!list) return;
  list.replaceChildren();
  if (state.repositoryLinkOperationsError) list.append(textNode(state.repositoryLinkOperationsError, "repository-link-inline-error"));
  if (!state.repositoryLinkOperations.length) {
    list.append(textNode(t(state.repositoryLinksStatus === "loading" ? "dynamic.loading" : "No creation history"), "repository-link-message"));
    return;
  }
  const stages = ["validating", "source", "root", "complete"];
  const labels = ["Validate", "Prepare Source", "Create Root link", "Complete"];
  for (const operation of state.repositoryLinkOperations) {
    let request;
    try { request = JSON.parse(operation.request); } catch (_) { request = {}; }
    const card = document.createElement("article"); card.className = "repository-link-operation"; card.dataset.operation = operation.id;
    card.append(textNode(`${request.source_repository ?? "?"} / ${request.source_path ?? "?"} → ${request.path ?? "?"}`, "repository-link-path"));
    const status = operation.status === "succeeded" ? "Complete" : operation.status === "partial" ? "Source ready · Root incomplete" : operation.status === "failed" ? "Creation failed" : "In progress";
    card.append(textNode(`${t(status)} · ${new Date(operation.updated_at).toLocaleString(state.locale)}`, "repository-link-target"));
    const steps = document.createElement("ol"); steps.className = "repository-link-steps";
    const active = stages.indexOf(operation.stage);
    labels.forEach((label, index) => {
      const step = document.createElement("li");
      step.textContent = t(label);
      step.className = index < active || operation.status === "succeeded" ? "is-complete" : index === active ? "is-active" : "";
      if (index === active && operation.status !== "succeeded") step.setAttribute("aria-current", "step");
      steps.append(step);
    });
    card.append(steps);
    if (operation.source_ready) card.append(textNode(t(operation.source_path_created ? "Source folder committed and pushed" : "Source verified"), "repository-link-target"));
    if (operation.error) card.append(textNode(operation.error, "repository-link-inline-error"));
    const stale = operation.status === "running" && Date.now() - new Date(operation.updated_at).getTime() > 30 * 60 * 1000;
    if (["failed", "partial"].includes(operation.status) || stale) {
      const retry = repositoryButton("retry", t(operation.source_ready ? "Retry Root link" : "Retry creation"));
      retry.disabled = state.repositoryLinksBusy;
      card.append(retry);
    } else if (operation.status === "running") {
      card.append(textNode(t("Refresh to check progress"), "repository-link-target"));
    }
    list.append(card);
  }
}

// Poll only durable progress, not expensive Lore checkouts, while a request is active.
function pollRepositoryLinkOperation(name, branch, id) {
  let polling = false;
  const timer = window.setInterval(async () => {
    if (polling) return;
    polling = true;
    try {
      await loadRepositoryLinkOperations(name, branch);
      const operation = state.repositoryLinkOperations.find(item => item.id === id);
      if (operation && elements["new-repository-link-dialog"].open) {
        const labels = { validating: "Validate", source: "Prepare Source", root: "Create Root link", complete: "Complete" };
        elements["repository-link-progress"].textContent = t(labels[operation.stage] ?? "In progress");
      }
    } finally { polling = false; }
  }, 2000);
  return () => window.clearInterval(timer);
}

async function createRepositoryLink(event) {
  event.preventDefault();
  if (state.repositoryLinksBusy) return;
  setRepositoryLinkFormError();
  const name = state.repositoryLinksName;
  const branch = state.repositoryLinksBranch;
  const input = {
    branch, expected_revision: state.repositoryLinksRevision,
    path: elements["repository-link-path"].value.trim(),
    source_repository: elements["repository-link-source-repository"].value,
    source_branch: elements["repository-link-source-branch"].value,
    source_path: elements["repository-link-source-path"].value.trim(),
    disable_branching: elements["repository-link-disable-branching"].checked,
    create_source_directory: elements["repository-link-create-source"].checked,
    auto_update: elements["repository-link-auto-update"].value === "true",
  };
  const parentLink = repositoryLinkContainingPath(input.path);
  if (parentLink) {
    setRepositoryLinkFormError(t("A LINK cannot be created below {path} because it is already a LINK.", { path: parentLink.path }));
    return;
  }
  // A lost response must not turn a second submit into a second operation.
  const signature = JSON.stringify({ name, ...input });
  const id = state.repositoryLinkDraft?.signature === signature ? state.repositoryLinkDraft.id : crypto.randomUUID();
  state.repositoryLinkDraft = { signature, id };
  input.operation_id = id;
  const controls = [...elements["repository-link-form"].querySelectorAll("input, select, button")];
  const disabled = controls.map(control => control.disabled);
  controls.forEach(control => { control.disabled = true; });
  elements["repository-link-progress"].hidden = false;
  elements["repository-link-progress"].textContent = t("Validate");
  const button = elements["create-repository-link-button"];
  button.querySelector("span:first-child").textContent = t("dynamic.saving");
  button.querySelector(".button-spinner").hidden = false;
  state.repositoryLinksBusy = true;
  renderRepositoryLinks();
  const stop = pollRepositoryLinkOperation(name, branch, id);
  try {
    const result = await api(`/api/v1/repositories/${encodeURIComponent(name)}/links`, {
      method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() }, body: JSON.stringify(input),
    });
    elements["new-repository-link-dialog"].close();
    toast(result.status === "succeeded" ? t(result.source_path_created ? "dynamic.linkAddedWithSourceFolder" : "dynamic.linkAdded") : result.error || t("In progress"), result.status === "succeeded" ? "success" : "error");
    if (state.repositoryLinksName === name && state.repositoryLinksBranch === branch) await loadRepositoryLinks();
  } catch (error) {
    setRepositoryLinkFormError(error.message);
    await loadRepositoryLinkOperations(name, branch);
    // If accepted by the server, recover via its operation instead of a second create.
    if (state.repositoryLinkOperations.some(item => item.id === id)) elements["new-repository-link-dialog"].close();
    toast(error.message || t("Request interrupted. Refresh the creation history before retrying."), "error");
  } finally {
    stop();
    state.repositoryLinksBusy = false;
    controls.forEach((control, index) => { control.disabled = disabled[index]; });
    button.querySelector("span:first-child").textContent = t("Add link");
    button.querySelector(".button-spinner").hidden = true;
    elements["repository-link-progress"].hidden = true;
    renderRepositoryLinks();
  }
}

async function retryRepositoryLink(event) {
  const button = event.target.closest('button[data-action="retry"]');
  const card = event.target.closest("[data-operation]");
  if (!button || !card || state.repositoryLinksBusy) return;
  if (!window.confirm(t("Retry checks the latest Root revision and preserves existing Source commits. Continue?"))) return;
  const name = state.repositoryLinksName;
  const branch = state.repositoryLinksBranch;
  state.repositoryLinksBusy = true;
  renderRepositoryLinks();
  const stop = pollRepositoryLinkOperation(name, branch, card.dataset.operation);
  try {
    const result = await api(`/api/v1/repositories/${encodeURIComponent(name)}/link-operations/${encodeURIComponent(card.dataset.operation)}/retry`, {
      method: "POST", headers: { "X-CSRF-Token": csrfToken() },
    });
    toast(result.status === "succeeded" ? t("dynamic.linkAdded") : result.error || t("In progress"), result.status === "succeeded" ? "success" : "error");
    if (state.repositoryLinksName === name && state.repositoryLinksBranch === branch) await loadRepositoryLinks();
  } catch (error) {
    toast(error.message, "error");
    await loadRepositoryLinkOperations(name, branch);
  } finally {
    stop(); state.repositoryLinksBusy = false; renderRepositoryLinks();
  }
}

function setRepositoryLinkFormError(message = "") {
  const error = elements["repository-link-form-error"];
  error.hidden = !message;
  error.querySelector("span").textContent = message;
}

async function performRepositoryLinkAction(action, path) {
  if (state.repositoryLinksBusy || !state.repositoryLinksRevision) return;
  const link = state.repositoryLinks.find(item => item.path === path);
  if (!link || !["update", "policy", "remove"].includes(action)) return;
  if (action === "remove" && !window.confirm(t("dynamic.removeLinkConfirm", { path }))) return;
  state.repositoryLinksBusy = true;
  renderRepositoryLinks();
  try {
    await api(`/api/v1/repositories/${encodeURIComponent(state.repositoryLinksName)}/links/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ branch: state.repositoryLinksBranch, expected_revision: state.repositoryLinksRevision, path, ...(action === "policy" ? { auto_update: !link.auto_update } : {}) }),
    });
    toast(t(action === "policy" ? "Sync policy saved" : action === "update" ? "dynamic.linkUpdated" : "dynamic.linkRemoved"), "success");
    await loadRepositoryLinks();
  } catch (error) {
    toast(error.message, "error");
  } finally {
    state.repositoryLinksBusy = false;
    renderRepositoryLinks();
  }
}

function openRepositoryConfig(repository) {
  navigateRepositorySection("ci-settings", repository.url);
}

async function loadCiSettings(notify) {
  const scope = state.repositoryScope;
  const preferredName = state.repositoryConfigName;
  resetRepositoryConfig();
  elements["repository-config-repository"].replaceChildren();
  elements["repository-config-branch"].replaceChildren();
  state.repositoryConfigStatus = "loading";
  renderRepositoryConfig();
  await loadRepositories(false);
  if (state.section !== "ci-settings" || scope !== state.repositoryScope) return;
  const candidates = await prioritizeCiSettingsRepositories();
  if (state.section !== "ci-settings" || scope !== state.repositoryScope) return;
  state.ciSettingsRepositories = candidates;
  await loadRepositoryConfigPage(preferredName);
  if (notify && state.repositoryConfigStatus !== "error") toast(t("dynamic.configRefreshed"), "success");
}

async function prioritizeCiSettingsRepositories() {
  return scopedRepositories();
}

async function loadRepositoryConfigPage(preferredName) {
  resetRepositoryConfig();
  elements["repository-config-repository"].replaceChildren();
  elements["repository-config-branch"].replaceChildren();
  for (const repository of state.ciSettingsRepositories) {
    elements["repository-config-repository"].add(new Option(repository.name, repository.name));
  }
  const repository = state.ciSettingsRepositories.find(item => item.name === preferredName) ?? state.ciSettingsRepositories[0];
  if (!repository) {
    state.repositoryConfigStatus = "no-repositories";
    renderRepositoryConfig();
    return;
  }
  state.repositoryConfigName = repository.name;
  state.repositoryConfigStatus = "loading";
  elements["repository-config-repository"].value = repository.name;
  const request = ++state.repositoryConfigRequest;
  renderRepositoryConfig();
  try {
    const branches = await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/branches`);
    if (request !== state.repositoryConfigRequest || state.repositoryConfigName !== repository.name) return;
    for (const branch of branches) elements["repository-config-branch"].add(new Option(branch.name, branch.name));
    if (!branches.length) {
      state.repositoryConfigStatus = "no-branches";
      renderRepositoryConfig();
      return;
    }
    elements["repository-config-branch"].value = selectRepositoryBranch(branches, repository.url).name;
    await loadRepositoryConfig();
  } catch (error) {
    if (request !== state.repositoryConfigRequest) return;
    state.repositoryConfigStatus = "error";
    state.repositoryConfigError = error.message;
    renderRepositoryConfig();
    toast(error.message, "error");
  }
}

async function loadRepositoryConfig() {
  const name = state.repositoryConfigName;
  const branch = elements["repository-config-branch"].value;
  if (!name || !branch) return;
  rememberRepositoryBranch(branch);
  ciElement("ci-list-search").value = "";
  const request = ++state.repositoryConfigRequest;
  state.repositoryConfigRevision = null;
  state.repositoryConfigContent = null;
  state.repositoryConfigModel = null;
  state.repositoryConfigDraft = null;
  state.repositoryConfigSelection = null;
  state.repositoryConfigEditing = false;
  state.repositoryConfigStatus = "loading";
  state.repositoryConfigError = "";
  renderRepositoryConfig();
  try {
    const config = await api(`/api/v1/repositories/${encodeURIComponent(name)}/ci-config?branch=${encodeURIComponent(branch)}`);
    if (request !== state.repositoryConfigRequest || state.repositoryConfigName !== name || elements["repository-config-branch"].value !== branch) return;
    state.repositoryConfigRevision = config.revision;
    state.repositoryConfigContent = config.content;
    state.repositoryConfigModel = config.configuration;
    state.repositoryConfigMode = config.configuration || config.content === null ? "visual" : "toml";
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: 0 };
    state.repositoryConfigStatus = config.is_link_source ? "link-source" : "ready";
    renderRepositoryConfig();
  } catch (error) {
    if (request !== state.repositoryConfigRequest) return;
    state.repositoryConfigStatus = "error";
    state.repositoryConfigError = error.message;
    renderRepositoryConfig();
    toast(error.message, "error");
  }
}

function renderRepositoryConfig() {
  updateCiWorkspaceControl();
  syncCiEditor();
  const editing = state.repositoryConfigEditing;
  const ready = state.repositoryConfigStatus === "ready";
  const missing = ready && state.repositoryConfigContent === null;
  elements["ci-config-template"].hidden = !missing || editing;
  elements["ci-config-template-source"].textContent = DEFAULT_CI_CONFIG;
  elements["ci-new-pipeline-button"].disabled = !ready || !state.repositoryScope || !state.repositoryBranch || editing || state.repositoryConfigSaving || state.repositoryConfigContent === null;
  const visual = ready && (!missing || editing) && state.repositoryConfigMode === "visual";
  const visualAvailable = ready && (state.repositoryConfigModel !== null || state.repositoryConfigContent === null || editing);
  const viewer = elements["repository-config-viewer"];
  let contents = "";
  if (state.repositoryConfigStatus === "loading") contents = t("dynamic.loadingConfig");
  else if (state.repositoryConfigStatus === "no-repositories") contents = t("dynamic.noRepositories");
  else if (state.repositoryConfigStatus === "no-branches") contents = t("dynamic.noRemoteBranches");
  else if (state.repositoryConfigStatus === "error") contents = state.repositoryConfigError;
  else if (state.repositoryConfigStatus === "link-source") contents = t("CI settings are unavailable for Lore link source repositories.");
  else if (state.repositoryConfigContent === null) contents = t("dynamic.noCiConfig");
  else contents = state.repositoryConfigContent;
  viewer.textContent = contents;
  viewer.classList.toggle("is-empty", !ready || state.repositoryConfigContent === null);
  viewer.hidden = visual || editing || missing;
  elements["repository-config-editor-field"].hidden = visual || !editing;
  elements["repository-config-mode"].hidden = !ready || (missing && !editing);
  elements["repository-config-visual"].hidden = !visual;
  elements["repository-config-visual-tab"].classList.toggle("is-active", visual);
  elements["repository-config-visual-tab"].setAttribute("aria-selected", String(visual));
  elements["repository-config-visual-tab"].disabled = !visualAvailable;
  elements["repository-config-toml-tab"].classList.toggle("is-active", ready && !visual);
  elements["repository-config-toml-tab"].setAttribute("aria-selected", String(ready && !visual));
  elements["repository-config-repository"].disabled = editing || state.repositoryConfigSaving || state.repositoryConfigStatus === "loading" || state.repositoryConfigStatus === "no-repositories";
  elements["repository-config-branch"].disabled = editing || state.repositoryConfigStatus === "loading" || state.repositoryConfigStatus === "no-branches";
  elements["repository-config-revision"].textContent = state.repositoryConfigRevision ?? "—";
  elements["repository-config-revision"].title = state.repositoryConfigRevision ?? "";
  elements["repository-config-edit"].hidden = editing || missing;
  elements["repository-config-edit"].disabled = !ready;
  elements["repository-config-edit"].textContent = t("Edit");
  elements["repository-config-cancel-edit"].hidden = !editing;
  elements["repository-config-save"].hidden = !editing;
  elements["repository-config-save"].disabled = state.repositoryConfigSaving;
  elements["repository-config-save"].querySelector("span:first-child").textContent = state.repositoryConfigSaving ? t("dynamic.saving") : t("Save changes");
  elements["repository-config-save"].querySelector(".button-spinner").hidden = !state.repositoryConfigSaving;
  if (visual) renderRepositoryConfigVisual();
  scheduleCiAnalysis();
  setCiSavingState();
  updateCiHistoryControls();
}

function setRepositoryConfigEditing(editing) {
  if (editing && state.repositoryConfigStatus !== "ready") return;
  state.repositoryConfigEditing = editing;
  if (editing) {
    state.repositoryConfigDraft = cloneCiModel(state.repositoryConfigModel ?? DEFAULT_CI_MODEL);
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: 0 };
    elements["repository-config-editor"].value = state.repositoryConfigContent ?? DEFAULT_CI_CONFIG;
  } else {
    state.repositoryConfigDraft = null;
    if (!state.repositoryConfigModel && state.repositoryConfigContent !== null) state.repositoryConfigMode = "toml";
  }
  renderRepositoryConfig();
  if (editing && state.repositoryConfigMode === "toml") window.setTimeout(() => elements["repository-config-editor"].focus(), 0);
}

async function setRepositoryConfigMode(mode) {
  if (state.repositoryConfigSaving) return;
  if (mode === state.repositoryConfigMode || state.repositoryConfigStatus !== "ready") return;
  if (mode === "toml") {
    if (state.repositoryConfigEditing && state.repositoryConfigDraft) {
      elements["repository-config-editor"].value = serializeCiModel(state.repositoryConfigDraft);
    }
    state.repositoryConfigMode = "toml";
    renderRepositoryConfig();
    if (state.repositoryConfigEditing) elements["repository-config-editor"].focus();
    return;
  }
  if (!state.repositoryConfigEditing) {
    if (!state.repositoryConfigModel && state.repositoryConfigContent !== null) return;
    state.repositoryConfigMode = "visual";
    renderRepositoryConfig();
    return;
  }
  const source = elements["repository-config-editor"].value;
  const generation = ciEditor.generation;
  const request = state.repositoryConfigRequest;
  try {
    const configuration = await api(`/api/v1/repositories/${encodeURIComponent(state.repositoryConfigName)}/ci-config/parse`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ content: source }),
    });
    if (!state.repositoryConfigEditing || state.repositoryConfigSaving || generation !== ciEditor.generation
      || request !== state.repositoryConfigRequest || state.repositoryConfigMode !== "toml"
      || elements["repository-config-editor"].value !== source) return;
    state.repositoryConfigDraft = configuration;
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: 0 };
    state.repositoryConfigMode = "visual";
    renderRepositoryConfig();
  } catch (error) {
    toast(error.message, "error");
  }
}

async function saveRepositoryConfig(event) {
  event.preventDefault();
  if (!state.repositoryConfigName || !state.repositoryConfigRevision || !state.repositoryConfigEditing || state.repositoryConfigSaving) return;
  const name = state.repositoryConfigName;
  const branch = elements["repository-config-branch"].value;
  const content = state.repositoryConfigMode === "visual"
    ? serializeCiModel(state.repositoryConfigDraft)
    : elements["repository-config-editor"].value;
  state.repositoryConfigSaving = true;
  renderRepositoryConfig();
  try {
    if (!await validateCiBeforeSave(content, name)) return;
    const config = await api(`/api/v1/repositories/${encodeURIComponent(name)}/ci-config`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ branch, expected_revision: state.repositoryConfigRevision, content }),
    });
    if (state.repositoryConfigName !== name || elements["repository-config-branch"].value !== branch) return;
    state.repositoryConfigRevision = config.revision;
    state.repositoryConfigContent = config.content;
    state.repositoryConfigModel = config.configuration;
    state.repositoryConfigDraft = null;
    state.repositoryConfigEditing = false;
    state.repositoryConfigStatus = "ready";
    toast(t("dynamic.configSaved"), "success");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    state.repositoryConfigSaving = false;
    renderRepositoryConfig();
  }
}

function resetRepositoryConfig() {
  closeCiWorkspace(false);
  ciElement("ci-list-search").value = "";
  resetCiEditor();
  state.repositoryConfigRequest += 1;
  state.repositoryConfigName = null;
  state.repositoryConfigRevision = null;
  state.repositoryConfigContent = null;
  state.repositoryConfigModel = null;
  state.repositoryConfigDraft = null;
  state.repositoryConfigMode = "visual";
  state.repositoryConfigSelection = null;
  state.repositoryConfigEditing = false;
  state.repositoryConfigSaving = false;
  state.repositoryConfigStatus = "idle";
  state.repositoryConfigError = "";
}

function discardRepositoryConfigEdit() {
  if (state.repositoryConfigEditing && !window.confirm(t("dynamic.discardConfig"))) return false;
  state.repositoryConfigEditing = false;
  state.repositoryConfigDraft = null;
  resetCiEditor();
  return true;
}

function cloneCiModel(model) {
  return JSON.parse(JSON.stringify(model));
}

function ciPipelineEntries(model) {
  if (model?.pipelines?.length) return model.pipelines.map((pipeline, pipelineIndex) => ({ pipeline, pipelineIndex, legacy: false }));
  if (!model) return [];
  return [{ pipeline: { name: t("Manual pipeline"), stages: model.stages ?? [], jobs: model.jobs ?? [] }, pipelineIndex: 0, legacy: true }];
}

function selectedCiPipeline(model) {
  const entries = ciPipelineEntries(model);
  if (!entries.length) return null;
  return entries.find(entry => entry.pipelineIndex === state.repositoryConfigSelection?.pipelineIndex) ?? entries[0];
}

function renderRepositoryConfigVisual() {
  recordCiEdit();
  const model = state.repositoryConfigEditing ? state.repositoryConfigDraft : state.repositoryConfigModel;
  const entries = ciPipelineEntries(model);
  const list = elements["repository-config-pipeline-list"];
  const graph = elements["repository-config-stage-graph"];
  const inspector = elements["repository-config-inspector"];
  list.replaceChildren();
  graph.replaceChildren();
  inspector.replaceChildren();
  graph.classList.toggle("ci-overview-graph", ciVisual.scope === "overview");
  updateCiGraphScope();
  applyCiZoom();
  scheduleCiAnalysis();
  elements["repository-config-pipeline-count"].textContent = String(entries.length);
  elements["repository-config-add-pipeline"].hidden = !state.repositoryConfigEditing;
  if (!entries.length) {
    filterCiPipelineList();
    graph.append(configEmptyState(t("No CI configuration"), t("Select Edit to create a pipeline graph.")));
    inspector.append(configEmptyState(t("Nothing selected"), t("Select a pipeline, stage, or job.")));
    elements["repository-config-graph-title"].textContent = t("Pipeline");
    elements["repository-config-inspector-title"].textContent = t("Pipeline settings");
    updateCiRevealControl();
    return;
  }

  const selected = selectedCiPipeline(model);
  if (!state.repositoryConfigSelection || selected.pipelineIndex !== state.repositoryConfigSelection.pipelineIndex) {
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: selected.pipelineIndex };
  }
  for (const entry of entries) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "config-pipeline-item";
    button.dataset.pipelineIndex = entry.pipelineIndex;
    button.classList.toggle("is-active", entry.pipelineIndex === selected.pipelineIndex);
    const name = document.createElement("strong"); name.textContent = entry.pipeline.name;
    const meta = document.createElement("span"); meta.textContent = entry.legacy ? t("Manual") : `${entry.pipeline.runner_os} · ${entry.pipeline.category}`;
    button.append(name, meta);
    if (entry.pipeline.needs?.length) {
      const needs = document.createElement("span"); needs.className = "config-pipeline-needs"; needs.textContent = t("dynamic.pipelineNeeds", { pipelines: entry.pipeline.needs.join(", ") }); button.append(needs);
    }
    button.addEventListener("click", () => {
      ciVisual.scope = "detail";
      state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: entry.pipelineIndex };
      renderRepositoryConfigVisual();
      window.requestAnimationFrame(() => revealCiSelection(false));
    });
    list.append(button);
  }

  filterCiPipelineList();
  elements["repository-config-add-pipeline"].textContent = selected.legacy ? t("Convert to auto pipeline") : t("Add pipeline");
  if (ciVisual.scope === "overview") {
    renderCiOverview(entries);
    renderConfigInspector(selected, model);
    decorateCiAnalysis();
    setCiSavingState();
    updateCiRevealControl();
    return;
  }
  elements["repository-config-graph-title"].textContent = selected.pipeline.name;
  selected.pipeline.stages.forEach((stage, stageIndex) => {
    const column = document.createElement("section");
    column.className = "config-stage-column";
    const header = document.createElement("button");
    header.type = "button";
    header.className = "config-stage-header";
    header.dataset.stageIndex = stageIndex;
    header.classList.toggle("is-active", state.repositoryConfigSelection?.type === "stage" && state.repositoryConfigSelection.stageIndex === stageIndex);
    const order = document.createElement("span"); order.textContent = String(stageIndex + 1).padStart(2, "0");
    const title = document.createElement("strong"); title.textContent = stage;
    header.append(order, title);
    header.addEventListener("click", () => {
      state.repositoryConfigSelection = { type: "stage", pipelineIndex: selected.pipelineIndex, stageIndex };
      renderRepositoryConfigVisual();
    });
    column.append(header);
    const jobs = selected.pipeline.jobs.map((job, jobIndex) => ({ job, jobIndex })).filter(entry => entry.job.stage === stage);
    const jobList = document.createElement("div"); jobList.className = "config-job-list";
    for (const entry of jobs) {
      const job = document.createElement("button");
      job.type = "button";
      job.className = "config-job-card";
      job.dataset.jobName = entry.job.name;
      job.dataset.jobIndex = entry.jobIndex;
      job.classList.toggle("is-active", state.repositoryConfigSelection?.type === "job" && state.repositoryConfigSelection.jobIndex === entry.jobIndex);
      const icon = document.createElement("span"); icon.className = "config-job-icon"; icon.textContent = "◆";
      const copy = document.createElement("span");
      const jobName = document.createElement("strong"); jobName.textContent = entry.job.name;
      const jobMeta = document.createElement("small"); jobMeta.textContent = t("dynamic.jobSteps", { count: entry.job.script.length, seconds: entry.job.timeout_seconds });
      copy.append(jobName, jobMeta);
      if (entry.job.needs?.length) {
        const needs = document.createElement("small"); needs.className = "config-job-needs"; needs.textContent = t("dynamic.jobNeeds", { jobs: entry.job.needs.join(", ") }); copy.append(needs);
      }
      job.append(icon, copy);
      job.addEventListener("click", () => {
        state.repositoryConfigSelection = { type: "job", pipelineIndex: selected.pipelineIndex, jobIndex: entry.jobIndex };
        renderRepositoryConfigVisual();
      });
      jobList.append(job);
    }
    if (state.repositoryConfigEditing) {
      const addJob = document.createElement("button"); addJob.type = "button"; addJob.className = "config-inline-add"; addJob.textContent = `＋ ${t("Add job")}`;
      addJob.addEventListener("click", () => addVisualJob(selected, stage));
      jobList.append(addJob);
    }
    column.append(jobList);
    graph.append(column);
  });
  if (state.repositoryConfigEditing) {
    const addStage = document.createElement("button"); addStage.type = "button"; addStage.className = "config-stage-add"; addStage.textContent = `＋ ${t("Add stage")}`;
    addStage.addEventListener("click", () => addVisualStage(selected));
    graph.append(addStage);
  }
  renderConfigInspector(selected, model);
  decorateCiAnalysis();
  setCiSavingState();
  updateCiRevealControl();
  window.requestAnimationFrame(() => renderConfigDependencyEdges(selected.pipeline));
}

function renderConfigInspector(selected, model) {
  const inspector = elements["repository-config-inspector"];
  const selection = state.repositoryConfigSelection ?? { type: "pipeline" };
  if (selection.type === "job") {
    const job = selected.pipeline.jobs[selection.jobIndex];
    if (!job) { state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: selected.pipelineIndex }; renderRepositoryConfigVisual(); return; }
    elements["repository-config-inspector-title"].textContent = t("Job settings");
    inspector.append(
      configInspectorInput(t("Name"), job.name, value => {
        const previous = job.name;
        job.name = value;
        for (const candidate of selected.pipeline.jobs) {
          candidate.needs = (candidate.needs ?? []).map(dependency => dependency === previous ? value : dependency);
        }
      }),
      configInspectorSelect(t("Stage"), selected.pipeline.stages, job.stage, value => {
        job.stage = value;
        for (const candidate of selected.pipeline.jobs) {
          const candidateStage = selected.pipeline.stages.indexOf(candidate.stage);
          candidate.needs = (candidate.needs ?? []).filter(dependency => {
            const dependencyJob = selected.pipeline.jobs.find(item => item.name === dependency);
            return dependencyJob && selected.pipeline.stages.indexOf(dependencyJob.stage) <= candidateStage;
          });
        }
      }),
      configInspectorDependencies(selected.pipeline, selection.jobIndex),
      configInspectorInput(t("Timeout (seconds)"), job.timeout_seconds, value => { job.timeout_seconds = Number(value); }, "number", { min: 1, max: 86400 }),
      configInspectorTextarea(t("Script"), job.script.join("\n"), value => { job.script = value.split("\n"); }, t("one command per line")),
    );
    if (state.repositoryConfigEditing) inspector.append(configDangerButton(t("Delete job"), () => deleteVisualJob(selected, selection.jobIndex)));
    return;
  }
  if (selection.type === "stage") {
    const stage = selected.pipeline.stages[selection.stageIndex];
    if (stage === undefined) { state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: selected.pipelineIndex }; renderRepositoryConfigVisual(); return; }
    elements["repository-config-inspector-title"].textContent = t("Stage settings");
    inspector.append(configInspectorInput(t("Name"), stage, value => {
      const previous = selected.pipeline.stages[selection.stageIndex];
      selected.pipeline.stages[selection.stageIndex] = value;
      for (const job of selected.pipeline.jobs) if (job.stage === previous) job.stage = value;
    }));
    if (state.repositoryConfigEditing) {
      const remove = configDangerButton(t("Delete stage"), () => deleteVisualStage(selected, selection.stageIndex));
      remove.disabled = selected.pipeline.stages.length <= 1;
      inspector.append(remove);
    }
    return;
  }

  elements["repository-config-inspector-title"].textContent = t("Pipeline settings");
  if (selected.legacy) {
    const note = document.createElement("p"); note.className = "config-inspector-note"; note.textContent = t("This is a manual pipeline using root stages and jobs.");
    inspector.append(note);
    if (state.repositoryConfigEditing) {
      const convert = document.createElement("button"); convert.type = "button"; convert.className = "button button--secondary"; convert.textContent = t("Convert to auto pipeline"); convert.addEventListener("click", addVisualPipeline); inspector.append(convert);
    }
    return;
  }
  const pipeline = selected.pipeline;
  inspector.append(
    configInspectorInput(t("Name"), pipeline.name, value => {
      const previous = pipeline.name;
      pipeline.name = value;
      for (const candidate of model.pipelines) {
        candidate.needs = (candidate.needs ?? []).map(dependency => dependency === previous ? value : dependency);
      }
    }),
    configInspectorInput(t("Category"), pipeline.category, value => { pipeline.category = value; }),
    configInspectorPipelineDependencies(model, selected.pipelineIndex),
    configInspectorSelect(t("Runner OS"), ["linux", "macos", "windows"], pipeline.runner_os, value => { pipeline.runner_os = value; }),
    configInspectorInput(t("Working directory"), pipeline.working_directory, value => { pipeline.working_directory = value; }),
    configInspectorTextarea(t("Change paths"), pipeline.changes.join("\n"), value => { pipeline.changes = value.split("\n").map(item => item.trim()).filter(Boolean); }, t("one path per line")),
    configInspectorInput(t("Sparse View"), pipeline.sparse_view ?? "", value => { pipeline.sparse_view = value.trim() || null; }),
  );
  if (state.repositoryConfigEditing) inspector.append(configDangerButton(t("Delete pipeline"), () => deleteVisualPipeline(selected.pipelineIndex)));
}

function configInspectorInput(labelText, value, update, type = "text", attributes = {}) {
  const label = document.createElement("label"); label.className = "config-inspector-field";
  const caption = document.createElement("span"); caption.textContent = labelText;
  const input = document.createElement("input"); input.type = type; input.value = value; input.disabled = !state.repositoryConfigEditing;
  input.dataset.configField = ciFieldKey(labelText);
  for (const [key, attributeValue] of Object.entries(attributes)) input[key] = attributeValue;
  input.addEventListener("input", () => { update(input.value); refreshCiGraphLabels(); });
  label.append(caption, input); return label;
}

function configInspectorSelect(labelText, options, value, update) {
  const label = document.createElement("label"); label.className = "config-inspector-field";
  const caption = document.createElement("span"); caption.textContent = labelText;
  const select = document.createElement("select"); select.disabled = !state.repositoryConfigEditing;
  select.dataset.configField = ciFieldKey(labelText);
  for (const option of options) select.add(new Option(option, option));
  select.value = value;
  select.addEventListener("change", () => { update(select.value); renderRepositoryConfigVisual(); });
  label.append(caption, select); return label;
}

function configInspectorTextarea(labelText, value, update, hintText) {
  const label = document.createElement("label"); label.className = "config-inspector-field";
  const caption = document.createElement("span"); caption.textContent = labelText;
  const textarea = document.createElement("textarea"); textarea.rows = 6; textarea.value = value; textarea.disabled = !state.repositoryConfigEditing; textarea.spellcheck = false;
  textarea.dataset.configField = ciFieldKey(labelText);
  textarea.addEventListener("input", () => { update(textarea.value); refreshCiGraphLabels(); });
  const hint = document.createElement("small"); hint.textContent = hintText;
  label.append(caption, textarea, hint); return label;
}

function configInspectorDependencies(pipeline, jobIndex) {
  const job = pipeline.jobs[jobIndex];
  job.needs ??= [];
  const field = document.createElement("fieldset"); field.className = "config-dependency-field";
  field.dataset.configField = "needs";
  const legend = document.createElement("legend"); legend.textContent = t("Dependencies"); field.append(legend);
  const hint = document.createElement("small"); hint.textContent = t("Select jobs that must complete first."); field.append(hint);
  const jobStage = pipeline.stages.indexOf(job.stage);
  const candidates = pipeline.jobs.filter(candidate => candidate !== job
    && pipeline.stages.indexOf(candidate.stage) <= jobStage
    && !wouldCreateCiDependencyCycle(pipeline, job.name, candidate.name));
  if (!candidates.length) {
    const empty = document.createElement("span"); empty.className = "config-dependency-empty"; empty.textContent = t("No eligible dependency jobs"); field.append(empty); return field;
  }
  const options = document.createElement("div"); options.className = "config-dependency-options";
  for (const candidate of candidates) {
    const option = document.createElement("label");
    const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.checked = job.needs.includes(candidate.name); checkbox.disabled = !state.repositoryConfigEditing;
    checkbox.addEventListener("change", () => {
      job.needs = checkbox.checked
        ? [...new Set([...job.needs, candidate.name])]
        : job.needs.filter(dependency => dependency !== candidate.name);
      renderRepositoryConfigVisual();
    });
    const name = document.createElement("strong"); name.textContent = candidate.name;
    const stage = document.createElement("small"); stage.textContent = candidate.stage;
    option.append(checkbox, name, stage); options.append(option);
  }
  field.append(options); return field;
}

function configInspectorPipelineDependencies(model, pipelineIndex) {
  const pipeline = model.pipelines[pipelineIndex];
  pipeline.needs ??= [];
  const field = document.createElement("fieldset"); field.className = "config-dependency-field";
  field.dataset.configField = "needs";
  const legend = document.createElement("legend"); legend.textContent = t("Pipeline dependencies"); field.append(legend);
  const hint = document.createElement("small"); hint.textContent = t("Select pipelines that must succeed first."); field.append(hint);
  const candidates = model.pipelines.filter(candidate => candidate !== pipeline
    && !wouldCreateCiPipelineDependencyCycle(model, pipeline.name, candidate.name));
  if (!candidates.length) {
    const empty = document.createElement("span"); empty.className = "config-dependency-empty"; empty.textContent = t("No eligible dependency pipelines"); field.append(empty); return field;
  }
  const options = document.createElement("div"); options.className = "config-dependency-options";
  for (const candidate of candidates) {
    const option = document.createElement("label");
    const checkbox = document.createElement("input"); checkbox.type = "checkbox"; checkbox.checked = pipeline.needs.includes(candidate.name); checkbox.disabled = !state.repositoryConfigEditing;
    checkbox.addEventListener("change", () => {
      pipeline.needs = checkbox.checked
        ? [...new Set([...pipeline.needs, candidate.name])]
        : pipeline.needs.filter(dependency => dependency !== candidate.name);
      renderRepositoryConfigVisual();
    });
    const name = document.createElement("strong"); name.textContent = candidate.name;
    const target = document.createElement("small"); target.textContent = candidate.runner_os;
    option.append(checkbox, name, target); options.append(option);
  }
  field.append(options); return field;
}

function wouldCreateCiPipelineDependencyCycle(model, pipelineName, dependencyName) {
  const pending = [dependencyName];
  const visited = new Set();
  while (pending.length) {
    const name = pending.pop();
    if (name === pipelineName) return true;
    if (visited.has(name)) continue;
    visited.add(name);
    const pipeline = model.pipelines.find(candidate => candidate.name === name);
    if (pipeline) pending.push(...(pipeline.needs ?? []));
  }
  return false;
}

function wouldCreateCiDependencyCycle(pipeline, jobName, dependencyName) {
  const pending = [dependencyName];
  const visited = new Set();
  while (pending.length) {
    const name = pending.pop();
    if (name === jobName) return true;
    if (visited.has(name)) continue;
    visited.add(name);
    const job = pipeline.jobs.find(candidate => candidate.name === name);
    if (job) pending.push(...(job.needs ?? []));
  }
  return false;
}

function renderConfigDependencyEdges(pipeline) {
  if (state.repositoryConfigMode !== "visual") return;
  if (ciVisual.scope === "overview") { renderCiOverviewEdges(); return; }
  const graph = elements["repository-config-stage-graph"];
  graph.querySelector(".config-dependency-layer")?.remove();
  const cards = new Map([...graph.querySelectorAll(".config-job-card")].map(card => [card.dataset.jobName, card]));
  const edges = pipeline.jobs.flatMap(job => (job.needs ?? []).map(dependency => ({ dependency, job: job.name })));
  if (!edges.length || !graph.isConnected) return;
  const width = Math.max(graph.scrollWidth, graph.clientWidth);
  const height = Math.max(graph.scrollHeight, graph.clientHeight);
  const namespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(namespace, "svg"); svg.classList.add("config-dependency-layer"); svg.setAttribute("width", width); svg.setAttribute("height", height); svg.setAttribute("viewBox", `0 0 ${width} ${height}`); svg.setAttribute("aria-hidden", "true");
  const defs = document.createElementNS(namespace, "defs");
  const marker = document.createElementNS(namespace, "marker"); marker.id = "config-dependency-arrow"; marker.setAttribute("viewBox", "0 0 10 10"); marker.setAttribute("refX", "9"); marker.setAttribute("refY", "5"); marker.setAttribute("markerWidth", "5"); marker.setAttribute("markerHeight", "5"); marker.setAttribute("orient", "auto-start-reverse");
  const arrow = document.createElementNS(namespace, "path"); arrow.setAttribute("d", "M 0 0 L 10 5 L 0 10 z"); marker.append(arrow); defs.append(marker); svg.append(defs);
  const graphRect = graph.getBoundingClientRect();
  const selected = state.repositoryConfigSelection?.type === "job" ? pipeline.jobs[state.repositoryConfigSelection.jobIndex]?.name : null;
  for (const edge of edges) {
    const source = cards.get(edge.dependency); const target = cards.get(edge.job);
    if (!source || !target) continue;
    const sourceRect = source.getBoundingClientRect(); const targetRect = target.getBoundingClientRect();
    const sameStage = source.closest(".config-stage-column") === target.closest(".config-stage-column");
    const x1 = (sourceRect.right - graphRect.left) / ciEditor.zoom + graph.scrollLeft;
    const y1 = (sourceRect.top + sourceRect.height / 2 - graphRect.top) / ciEditor.zoom + graph.scrollTop;
    const x2 = ((sameStage ? targetRect.right : targetRect.left) - graphRect.left) / ciEditor.zoom + graph.scrollLeft;
    const y2 = (targetRect.top + targetRect.height / 2 - graphRect.top) / ciEditor.zoom + graph.scrollTop;
    const path = document.createElementNS(namespace, "path");
    if (sameStage) {
      const control = Math.max(x1, x2) + 22;
      path.setAttribute("d", `M ${x1} ${y1} C ${control} ${y1}, ${control} ${y2}, ${x2} ${y2}`);
    } else {
      const middle = (x1 + x2) / 2;
      path.setAttribute("d", `M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}`);
    }
    path.setAttribute("marker-end", "url(#config-dependency-arrow)");
    path.classList.toggle("is-active", selected === edge.job || selected === edge.dependency);
    svg.append(path);
  }
  graph.prepend(svg);
}

function configDangerButton(label, action) {
  const button = document.createElement("button"); button.type = "button"; button.className = "button button--danger config-inspector-danger"; button.textContent = label; button.addEventListener("click", action); return button;
}

function configEmptyState(titleText, description) {
  const empty = document.createElement("div"); empty.className = "config-visual-empty";
  const title = document.createElement("strong"); title.textContent = titleText;
  const copy = document.createElement("span"); copy.textContent = description;
  empty.append(title, copy); return empty;
}

function addVisualPipeline() {
  if (!state.repositoryConfigEditing || !state.repositoryConfigDraft) return;
  const model = state.repositoryConfigDraft;
  if (!model.pipelines.length) {
    model.pipelines = [{
      name: "pipeline", category: "uncategorized", needs: [], runner_os: "linux", sparse_view: null,
      changes: ["src/**"], working_directory: ".", stages: model.stages, jobs: model.jobs,
    }];
    model.stages = []; model.jobs = [];
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: 0 };
  } else {
    const name = uniqueCiName(model.pipelines.map(pipeline => pipeline.name), "pipeline");
    model.pipelines.push({
      name, category: "uncategorized", needs: [], runner_os: "linux", sparse_view: null,
      changes: ["src/**"], working_directory: ".", stages: ["build"],
      jobs: [{ name: "build", stage: "build", needs: [], script: ["echo build"], timeout_seconds: 3600 }],
    });
    state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: model.pipelines.length - 1 };
  }
  renderRepositoryConfigVisual();
}

function deleteVisualPipeline(index) {
  const model = state.repositoryConfigDraft;
  const [deleted] = model.pipelines.splice(index, 1);
  for (const pipeline of model.pipelines) pipeline.needs = (pipeline.needs ?? []).filter(dependency => dependency !== deleted.name);
  if (!model.pipelines.length) {
    model.stages = cloneCiModel(DEFAULT_CI_MODEL.stages);
    model.jobs = cloneCiModel(DEFAULT_CI_MODEL.jobs);
  }
  state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: 0 };
  renderRepositoryConfigVisual();
}

function addVisualStage(selected) {
  const name = uniqueCiName(selected.pipeline.stages, "stage");
  selected.pipeline.stages.push(name);
  state.repositoryConfigSelection = { type: "stage", pipelineIndex: selected.pipelineIndex, stageIndex: selected.pipeline.stages.length - 1 };
  renderRepositoryConfigVisual();
}

function deleteVisualStage(selected, stageIndex) {
  const [stage] = selected.pipeline.stages.splice(stageIndex, 1);
  const deleted = new Set(selected.pipeline.jobs.filter(job => job.stage === stage).map(job => job.name));
  for (let index = selected.pipeline.jobs.length - 1; index >= 0; index -= 1) {
    if (selected.pipeline.jobs[index].stage === stage) selected.pipeline.jobs.splice(index, 1);
  }
  for (const job of selected.pipeline.jobs) job.needs = (job.needs ?? []).filter(dependency => !deleted.has(dependency));
  state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: selected.pipelineIndex };
  renderRepositoryConfigVisual();
}

function addVisualJob(selected, stage) {
  const name = uniqueCiName(selected.pipeline.jobs.map(job => job.name), "job");
  selected.pipeline.jobs.push({ name, stage, needs: [], script: ["echo build"], timeout_seconds: 3600 });
  state.repositoryConfigSelection = { type: "job", pipelineIndex: selected.pipelineIndex, jobIndex: selected.pipeline.jobs.length - 1 };
  renderRepositoryConfigVisual();
}

function deleteVisualJob(selected, jobIndex) {
  const [deleted] = selected.pipeline.jobs.splice(jobIndex, 1);
  for (const job of selected.pipeline.jobs) job.needs = (job.needs ?? []).filter(dependency => dependency !== deleted.name);
  state.repositoryConfigSelection = { type: "pipeline", pipelineIndex: selected.pipelineIndex };
  renderRepositoryConfigVisual();
}

function uniqueCiName(values, prefix) {
  let suffix = 1;
  let candidate = prefix;
  while (values.includes(candidate)) candidate = `${prefix}-${++suffix}`;
  return candidate;
}

function serializeCiModel(model) {
  const lines = [];
  const value = input => JSON.stringify(String(input));
  const array = items => `[${items.map(item => value(item)).join(", ")}]`;
  const appendJob = (job, table) => {
    lines.push(`[[${table}]]`, `name = ${value(job.name)}`, `stage = ${value(job.stage)}`);
    if (job.needs?.length) lines.push(`needs = ${array(job.needs)}`);
    lines.push(`script = ${array(job.script)}`, `timeout_seconds = ${Number.isFinite(Number(job.timeout_seconds)) ? Number(job.timeout_seconds) : 0}`, "");
  };
  if (model.pipelines.length) {
    for (const pipeline of model.pipelines) {
      lines.push("[[pipelines]]", `name = ${value(pipeline.name)}`, `category = ${value(pipeline.category)}`, `runner_os = ${value(pipeline.runner_os)}`);
      if (pipeline.needs?.length) lines.push(`needs = ${array(pipeline.needs)}`);
      if (pipeline.sparse_view) lines.push(`sparse_view = ${value(pipeline.sparse_view)}`);
      lines.push(`changes = ${array(pipeline.changes)}`, `working_directory = ${value(pipeline.working_directory)}`, `stages = ${array(pipeline.stages)}`, "");
      for (const job of pipeline.jobs) appendJob(job, "pipelines.jobs");
    }
  } else {
    lines.push(`stages = ${array(model.stages)}`, "");
    for (const job of model.jobs) appendJob(job, "jobs");
  }
  return `${lines.join("\n").trim()}\n`;
}

function openNewRepository() {
  elements["repository-form"].reset();
  for (const input of document.querySelectorAll('input[name="storage_backend"]')) {
    const available = state.repositoryStorageBackends.includes(input.value);
    input.disabled = !available;
    input.closest(".storage-backend-option").setAttribute("aria-disabled", String(!available));
  }
  const selected = document.querySelector('input[name="storage_backend"]:checked:not(:disabled)')
    ?? document.querySelector('input[name="storage_backend"]:not(:disabled)');
  if (selected) selected.checked = true;
  elements["new-repository-dialog"].showModal();
  window.setTimeout(() => elements["repository-name"].focus(), 0);
}

async function createRepository(event) {
  event.preventDefault();
  const button = elements["create-repository-button"];
  button.disabled = true;
  button.querySelector("span:first-child").textContent = t("dynamic.creating");
  button.querySelector(".button-spinner").hidden = false;
  try {
    const description = elements["repository-description"].value.trim();
    const storageBackend = document.querySelector('input[name="storage_backend"]:checked')?.value ?? "dynamodb_s3";
    await api("/api/v1/repositories", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({ name: elements["repository-name"].value.trim(), description: description || null, storage_backend: storageBackend }),
    });
    elements["new-repository-dialog"].close();
    toast(t("dynamic.repositoryCreated"), "success");
    await loadRepositories(false);
  } catch (error) { toast(error.message, "error"); }
  finally {
    button.disabled = false;
    button.querySelector("span:first-child").textContent = t("Create repository");
    button.querySelector(".button-spinner").hidden = true;
  }
}

async function deleteRepository(event) {
  event.preventDefault();
  const name = state.repositoryToDelete;
  if (!name || elements["delete-repository-confirmation"].value !== name) {
    elements["delete-repository-confirmation"].setCustomValidity(t("dynamic.nameMismatch"));
    elements["delete-repository-confirmation"].reportValidity();
    return;
  }
  elements["delete-repository-confirmation"].setCustomValidity("");
  elements["confirm-delete-repository-button"].disabled = true;
  try {
    await api(`/api/v1/repositories/${encodeURIComponent(name)}/delete`, { method: "POST", headers: { "X-CSRF-Token": csrfToken() } });
    elements["delete-repository-dialog"].close();
    state.repositoryToDelete = null;
    toast(t("dynamic.repositoryDeleted"), "success");
    await loadRepositories(false);
  } catch (error) { toast(error.message, "error"); }
  finally { elements["confirm-delete-repository-button"].disabled = false; }
}

async function issueLoreToken() {
  elements["create-lore-token-button"].disabled = true;
  try {
    const token = await api("/api/v1/lore-token", {
      method: "POST",
      headers: { "X-CSRF-Token": csrfToken() },
    });
    elements["lore-access-token"].value = token.access_token;
    elements["lore-token-dialog"].showModal();
  } catch (error) { toast(error.message, "error"); }
  finally { elements["create-lore-token-button"].disabled = false; }
}

async function copyLoreToken() {
  try {
    await navigator.clipboard.writeText(elements["lore-access-token"].value);
    toast(t("dynamic.tokenCopied"), "success");
  } catch (_) { toast(t("dynamic.copyFailed"), "error"); }
}

function closeLoreToken() {
  elements["lore-token-dialog"].close();
  elements["lore-access-token"].value = "";
}

function filteredPipelines() {
  return state.pipelines;
}

function renderPipelines() {
  const exactStatus = state.filter === "queued" ? "queued" : state.filter === "executing" ? "running" : "";
  document.getElementById("pipeline-panel-title").textContent = state.section === "overview" ? ovt("runs") : `${t("Recent pipelines")}${exactStatus ? ` · ${statusLabel(exactStatus)}` : ""}`;
  document.getElementById("pipeline-panel-description").textContent = state.section === "overview" ? ovt("runsNote") : t("최신 Lore revision 실행 내역");
  updatePipelineFilterOptions();
  const pipelines = filteredPipelines();
  const loading = state.pipelineLoading || Boolean(state.pipelineSearchTimer);
  const filtered = ["q", "branch", "pipeline_name", "status"].some(key => pipelineHistoryParameters().has(key));
  const empty = elements["empty-state"];
  empty.querySelector("h3").textContent = filtered ? rct("noMatches") : t("파이프라인이 없습니다");
  empty.querySelector("p").textContent = filtered ? rct("changeFilters") : t("Select a repository and open CI settings to create a pipeline run.");
  document.querySelectorAll(".status-tab").forEach(tab => tab.setAttribute("aria-pressed", String(tab.dataset.status === state.filter)));
  elements["pipeline-table-body"].replaceChildren();
  elements["empty-state"].hidden = loading || pipelines.length !== 0;
  document.querySelector(".table-wrap").hidden = pipelines.length === 0;
  for (const pipeline of pipelines) {
    const row = document.createElement("tr");
    row.dataset.id = pipeline.id;
    row.tabIndex = 0;
    row.setAttribute("aria-label", t("dynamic.openPipeline", { repository: repositoryName(pipeline.repository_url) }));
    row.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); void openPipeline(pipeline.id); } });
    row.append(
      cell(statusBadge(pipeline.status)),
      pipelineCell(pipeline),
      cell(textNode(revisionLabel(pipeline), "revision")),
      cell(textNode(relativeTime(pipeline.created_at), "time-cell")),
      cell(textNode(duration(pipeline.started_at, pipeline.finished_at), "time-cell")),
      cell(actionButton()),
    );
    elements["pipeline-table-body"].append(row);
  }
  const label = pipelines.length === state.pipelines.length
    ? tc("dynamic.countPipelines", pipelines.length)
    : t("dynamic.countFilteredPipelines", { shown: pipelines.length, total: state.pipelines.length });
  elements["pipeline-count"].textContent = loading ? t("dynamic.loading") : label;
}

function updatePipelineFilterOptions() {
  const repositoryValues = [...new Set([...state.repositories.map(repo => repo.url), ...state.pipelines.map(pipeline => pipeline.repository_url), ...(state.repositoryScope ? [state.repositoryScope] : [])])].sort();
  replacePipelineFilterOptions(
    elements["pipeline-repository-filter"],
    t("All repositories"),
    repositoryValues.map((url) => ({ value: url, label: repositoryName(url) })),
    "pipelineRepositoryFilter",
  );
  const repositoryPipelines = state.pipelines.filter((pipeline) => !state.pipelineRepositoryFilter || pipeline.repository_url === state.pipelineRepositoryFilter);
  const branchValues = uniquePipelineValues(repositoryPipelines, (pipeline) => pipeline.branch);
  elements["pipeline-branch-filter"].value = state.pipelineBranchFilter;
  document.getElementById("pipeline-branch-suggestions").replaceChildren(...branchValues.map(value => new Option(value, value)));
  const branchPipelines = repositoryPipelines.filter((pipeline) => !state.pipelineBranchFilter || pipeline.branch === state.pipelineBranchFilter);
  const pipelineValues = uniquePipelineValues(branchPipelines, (pipeline) => pipeline.pipeline_name);
  elements["pipeline-name-filter"].value = state.pipelineNameFilter;
  document.getElementById("pipeline-name-suggestions").replaceChildren(...pipelineValues.map(value => new Option(value, value)));
  document.getElementById("pipeline-search-hint").textContent = rct(state.repositoryScope ? "scopedSearchHint" : "searchHint");
}

function uniquePipelineValues(pipelines, valueFor) {
  return Array.from(new Set(pipelines.map(valueFor).filter(Boolean)))
    .sort((left, right) => left.localeCompare(right, localeTag()));
}

function replacePipelineFilterOptions(select, allLabel, options, stateKey) {
  select.replaceChildren(new Option(allLabel, ""));
  for (const option of options) select.add(new Option(option.label, option.value));
  if (!options.some((option) => option.value === state[stateKey])) state[stateKey] = "";
  select.value = state[stateKey];
}

function pipelineCell(pipeline) {
  const td = document.createElement("td");
  td.className = "pipeline-cell";
  const name = document.createElement("div");
  name.className = "pipeline-name";
  name.append(textNode(`#${pipeline.id.slice(0, 8)}`, "pipeline-id"), document.createTextNode(repositoryName(pipeline.repository_url)));
  if (pipeline.pipeline_name) name.append(document.createTextNode(` / ${pipeline.pipeline_name} · ${pipeline.runner_os}`));
  let source = pipeline.branch ? `${t("Branch")} · ${pipeline.branch}` : t("dynamic.manualRun");
  source += ` · ${pipeline.sparse_view_name ? `${t("Sparse View")} · ${pipeline.sparse_view_name}` : t("dynamic.noSparseView")}`;
  td.append(name, textNode(source, "pipeline-repo"));
  return td;
}

function cell(content) { const td = document.createElement("td"); td.append(content); return td; }
function textNode(value, className) { const span = document.createElement("span"); span.className = className; span.textContent = value; return span; }
function actionButton() { const button = document.createElement("button"); button.type = "button"; button.className = "row-action"; button.setAttribute("aria-label", t("dynamic.detail")); button.textContent = "•••"; return button; }

function statusBadge(status) {
  const badge = document.createElement("span");
  badge.className = `status-badge status-badge--${status}`;
  const dot = document.createElement("i"); dot.className = "status-dot";
  badge.append(dot, document.createTextNode(statusLabel(status)));
  return badge;
}

async function openNewPipeline(repositoryUrl = "", preferredBranch = "") {
  if (typeof repositoryUrl !== "string") repositoryUrl = "";
  repositoryUrl ||= state.repositoryScope;
  elements["pipeline-form"].reset();
  elements["repository-url"].disabled = true;
  elements.branch.disabled = true;
  elements.revision.value = "";
  elements["pipeline-name"].replaceChildren();
  elements["pipeline-name"].disabled = true;
  elements["pipeline-name"].dataset.ready = "";
  elements["run-pipeline-button"].disabled = true;
  elements["new-pipeline-dialog"].showModal();
  try {
    if (!state.repositories.length) {
      const payload = await api("/api/v1/repositories");
      state.repositories = payload.repositories;
    repositoryListStatus = "ready";
    reconcileRepositorySelection();
      state.repositoryServerUrl = payload.server_url;
      state.repositoryStorageBackends = payload.storage_backends ?? ["dynamodb_s3"];
    }
    elements["repository-url"].replaceChildren();
    if (!state.repositories.length) {
      elements["repository-url"].append(new Option(t("dynamic.noRepositories"), ""));
      return;
    }
    for (const repository of state.repositories) {
      elements["repository-url"].append(new Option(repository.name, repository.url));
    }
    elements["repository-url"].value = repositoryUrl || state.repositories[0].url;
    elements["repository-url"].disabled = false;
    await loadPipelineBranches(preferredBranch);
    window.setTimeout(() => elements["repository-url"].focus(), 0);
  } catch (error) {
    toast(error.message, "error");
  }
}

async function loadPipelineBranches(preferredBranch = "") {
  const repositoryUrl = elements["repository-url"].value;
  const repository = state.repositories.find((item) => item.url === repositoryUrl);
  elements.branch.replaceChildren(new Option(t("dynamic.loadingBranches"), ""));
  elements.branch.disabled = true;
  elements.revision.value = "";
  elements["pipeline-name"].replaceChildren();
  elements["pipeline-name"].disabled = true;
  elements["pipeline-name"].dataset.ready = "";
  elements["run-pipeline-button"].disabled = true;
  if (!repository) return;
  try {
    const branches = await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/branches`);
    if (elements["repository-url"].value !== repositoryUrl) return;
    elements.branch.replaceChildren();
    if (!branches.length) {
      elements.branch.append(new Option(t("dynamic.noBranches"), ""));
      return;
    }
    for (const branch of branches) {
      const option = new Option(branch.name, branch.name);
      option.dataset.revision = branch.revision;
      elements.branch.append(option);
    }
    if (preferredBranch && !branches.some(branch => branch.name === preferredBranch)) {
      elements.branch.value = "";
      toast(t("Selected branch is no longer available. Refresh CI settings."), "error");
      return;
    }
    const preferred = preferredBranch || repositoryBranchFor(repositoryUrl);
    elements.branch.value = branches.some(branch => branch.name === preferred) ? preferred : branches.some(branch => branch.name === "main") ? "main" : branches[0].name;
    elements.branch.disabled = false;
    await selectPipelineBranch();
  } catch (error) {
    if (elements["repository-url"].value === repositoryUrl) toast(error.message, "error");
  }
}

async function selectPipelineBranch() {
  const option = elements.branch.selectedOptions[0];
  const revision = option?.dataset.revision || "";
  const repositoryUrl = elements["repository-url"].value;
  const repository = state.repositories.find((item) => item.url === repositoryUrl);
  elements.revision.value = revision;
  elements["pipeline-name"].replaceChildren(new Option(t("dynamic.loadingPipelines"), ""));
  elements["pipeline-name"].disabled = true;
  elements["pipeline-name"].dataset.ready = "";
  elements["run-pipeline-button"].disabled = true;
  if (!repository || !revision) return;
  try {
    const choices = await api(`/api/v1/repositories/${encodeURIComponent(repository.name)}/pipelines?revision=${encodeURIComponent(revision)}`);
    if (elements["repository-url"].value !== repositoryUrl || elements.revision.value !== revision) return;
    elements["pipeline-name"].replaceChildren();
    if (!choices.length) {
      elements["pipeline-name"].append(new Option(t("dynamic.noPipelines"), ""));
      return;
    }
    for (const choice of choices) {
      const root = choice.name === null;
      const label = root ? t("dynamic.defaultPipeline") : `${choice.category} / ${choice.name} · ${choice.runner_os}`;
      const pipelineOption = new Option(label, choice.name || "");
      pipelineOption.dataset.root = root ? "true" : "";
      elements["pipeline-name"].append(pipelineOption);
    }
    elements["pipeline-name"].disabled = choices.length === 1 && choices[0].name === null;
    elements["pipeline-name"].dataset.ready = "true";
    updatePipelineRunAvailability();
  } catch (error) {
    if (elements["repository-url"].value === repositoryUrl && elements.revision.value === revision) toast(error.message, "error");
  }
}

function updatePipelineRunAvailability() {
  elements["run-pipeline-button"].disabled = !elements.revision.value || elements["pipeline-name"].dataset.ready !== "true";
}

async function submitPipeline(event) {
  event.preventDefault();
  const repositoryUrl = elements["repository-url"].value.trim();
  if (!/^lores:\/\//i.test(repositoryUrl)) {
    elements["repository-url"].setCustomValidity(t("dynamic.invalidLoreUrl"));
    elements["repository-url"].reportValidity();
    return;
  }
  elements["repository-url"].setCustomValidity("");
  setSubmitting(true);
  try {
    const pipeline = await api("/api/v1/pipelines", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify({
        repository_url: repositoryUrl,
        branch: elements.branch.value,
        revision: elements.revision.value.trim(),
        pipeline_name: elements["pipeline-name"].selectedOptions[0]?.dataset.root === "true"
          ? undefined
          : elements["pipeline-name"].value,
      }),
    });
    elements["new-pipeline-dialog"].close();
    toast(t("dynamic.pipelineQueued"), "success");
    await loadPipelines(false);
    await openPipeline(pipeline.id);
  } catch (error) {
    toast(error.message, "error");
  } finally {
    setSubmitting(false);
  }
}

function setSubmitting(submitting) {
  if (submitting) elements["run-pipeline-button"].disabled = true;
  else updatePipelineRunAvailability();
  elements["run-pipeline-button"].querySelector("span:first-child").textContent = submitting ? t("dynamic.starting") : t("Run pipeline");
  elements["run-pipeline-button"].querySelector(".button-spinner").hidden = !submitting;
}

const detailLogCopy = {
  more: ["로그 500건 더 보기", "Load 500 more records", "再加载 500 条记录"],
  retry: ["로그 다시 시도", "Retry logs", "重试日志"],
  follow: ["새 출력 따라가기", "Follow new output", "跟随新输出"],
  restart: ["처음부터 보기", "Start from the beginning", "从头查看"],
  detailRetry: ["상세 다시 시도", "Retry detail", "重试详情"],
  copyLink: ["실행 링크 복사", "Copy run link", "复制运行链接"],
  openLink: ["실행 링크 열기", "Open run link", "打开运行链接"],
  paused: ["자동 갱신과 스크롤이 일시 정지되었습니다.", "Automatic updates and scrolling are paused.", "已暂停自动更新和滚动。"],
  pending: ["뒤에 로그가 더 있을 수 있습니다. 더 보기를 눌러 계속 읽으세요.", "More records may follow. Load more to continue reading.", "后面可能还有日志，请继续加载。"],
  trimmed: ["화면에는 최근에 읽은 로그 일부만 유지합니다. 이전 부분은 ‘처음부터 보기’로 다시 읽을 수 있습니다.", "Only the most recently loaded output is kept on screen. Restart to read earlier output again.", "仅保留最近加载的输出，可从头重新查看较早的内容。"],
};
function dlt(key) { return detailLogCopy[key][state.locale === "ko" ? 0 : state.locale === "zh-CN" ? 2 : 1]; }

function resetDetailLogs(id, prunedAt = null) {
  state.detailLogView = { id, prunedAt, rows: [], after: 0, more: true, started: false, busy: false, follow: true, trimmed: false, error: null };
  return state.detailLogView;
}

async function restartDetailLogs() {
  const cache = state.detailLogView;
  if (!cache || cache.busy) return;
  resetDetailLogs(cache.id, cache.prunedAt).follow = false;
  elements["pipeline-log"].scrollTop = 0;
  await loadPipelineLogs(cache.id);
}

function clearPipelineDetail(message) {
  state.detailLogView = null;
  elements["detail-title"].textContent = "";
  elements["detail-repository"].textContent = "";
  elements["detail-summary"].replaceChildren();
  elements["execution-graph"].replaceChildren();
  elements["execution-graph-section"].hidden = true;
  elements["job-list"].replaceChildren(textNode(message, "job-empty"));
  elements["job-count"].textContent = "";
  elements["cancel-pipeline-button"].hidden = true;
  renderLogs([], null);
}

async function openPipeline(id, { fromLocation = false } = {}) {
  if (!fromLocation) recordPipelineLocation(id);
  state.selectedId = id;
  clearPipelineDetail(t("dynamic.loading"));
  resetDetailLogs(id);
  elements["detail-retry"].hidden = true;
  if (!elements["pipeline-detail-dialog"].open) elements["pipeline-detail-dialog"].showModal();
  elements["detail-title"].textContent = t("dynamic.loading");
  renderPipelinePermalink(id);
  try { await loadPipelineDetail(id); } catch (error) { toast(error.message, "error"); }
}

function renderPipelinePermalink(id) {
  elements["detail-copy-link"].textContent = dlt("copyLink");
  elements["detail-permalink"].textContent = dlt("openLink");
  elements["detail-permalink"].href = pipelinePermalink(id);
}

async function loadPipelineDetail(id) {
  const request = ++executionDetailRequest;
  let detail;
  try { detail = await api(`/api/v1/pipelines/${encodeURIComponent(id)}`); }
  catch (error) {
    if (state.selectedId !== id || request !== executionDetailRequest) return;
    clearPipelineDetail(error.message);
    elements["detail-title"].textContent = t("dynamic.requestFailed", { status: error.status || "—" });
    elements["detail-retry"].hidden = false;
    elements["detail-retry"].textContent = dlt("detailRetry");
    throw error;
  }
  if (state.selectedId !== id || request !== executionDetailRequest) return;
  renderPipelinePermalink(id);
  const pipeline = detail.pipeline;
  const prunedAt = pipeline.logs_pruned_at || null;
  if (state.detailLogView?.id !== id || state.detailLogView.prunedAt !== prunedAt) resetDetailLogs(id, prunedAt);
  elements["detail-retry"].hidden = true;
  elements["detail-repository"].textContent = [
    repositoryName(pipeline.repository_url),
    pipeline.branch,
    pipeline.category && pipelineCategory(pipeline),
  ].filter(Boolean).join(" / ");
  elements["detail-title"].textContent = pipeline.pipeline_name || revisionLabel(pipeline);
  renderDetailSummary(pipeline, detail.sparse_view_rules);
  renderExecutionGraph(pipeline, detail.jobs, detail.graph, detail.queue_reason);
  renderJobs(detail.jobs);
  const cancellable = ["queued", "running"].includes(pipeline.status) && !pipeline.cancel_requested;
  elements["cancel-pipeline-button"].hidden = !cancellable;
  elements["cancel-pipeline-button"].disabled = false;
  const cache = state.detailLogView;
  renderLogs(cache.rows, cache.prunedAt);
  // Summary and jobs are visible before the first log page resolves. A full page
  // requires an explicit next-page request instead of draining an unbounded log.
  if (!cache.started || (cache.follow && !cache.more && !cache.error)) await loadPipelineLogs(id);
}

function appendDetailLogs(cache, rows) {
  cache.after = rows.at(-1)?.id ?? cache.after;
  cache.more = rows.length === 500;
  cache.started = true;
  cache.rows.push(...rows);
  let size = cache.rows.reduce((total, row) => total + row.content.length, 0);
  while (cache.rows.length > 1 && (cache.rows.length > 2000 || size > 512000)) {
    size -= cache.rows.shift().content.length;
    cache.trimmed = true;
  }
  if (size > 512000) {
    cache.rows[0] = { ...cache.rows[0], content: cache.rows[0].content.slice(-512000) };
    cache.trimmed = true;
  }
}

async function loadPipelineLogs(id) {
  const cache = state.detailLogView;
  if (!cache || cache.id !== id || state.selectedId !== id || cache.busy) return;
  cache.busy = true;
  cache.error = null;
  renderLogs(cache.rows, cache.prunedAt);
  try {
    const rows = await api(`/api/v1/pipelines/${encodeURIComponent(id)}/logs?after=${cache.after}&limit=500`);
    if (state.detailLogView !== cache || state.selectedId !== id) return;
    appendDetailLogs(cache, rows);
  } catch (error) {
    if (state.detailLogView !== cache || state.selectedId !== id) return;
    if ([403, 404].includes(error.status)) {
      clearPipelineDetail(error.message);
      elements["detail-retry"].hidden = false;
      elements["detail-retry"].textContent = dlt("detailRetry");
      return;
    }
    cache.error = error.message;
    cache.started = true;
  } finally {
    cache.busy = false;
    if (state.detailLogView === cache && state.selectedId === id) renderLogs(cache.rows, cache.prunedAt);
  }
}

function renderExecutionGraph(pipeline, jobs, snapshot, queueReason) {
  const section = elements["execution-graph-section"];
  const graph = elements["execution-graph"];
  if (!pipeline.pipeline_name) {
    section.hidden = true;
    graph.replaceChildren();
    return;
  }

  section.hidden = false;
  renderExecutionWorkspace(graph, { pipeline, jobs, graph: snapshot, queue_reason: queueReason }, `drawer:${pipeline.id}`);
}

function osLabel(os) {
  return { windows: "Windows", macos: "macOS", linux: "Linux" }[os] || os || t("Any OS");
}

function revisionLabel(pipeline) {
  return `#${pipeline.revision_number}`;
}

function renderDetailSummary(pipeline, sparseViewRules) {
  elements["detail-summary"].replaceChildren();
  const values = [
    [t("Status"), statusLabel(pipeline.status)],
    [t("Revision"), revisionLabel(pipeline)],
    [t("Created"), formatDate(pipeline.created_at)],
    [t("Duration"), duration(pipeline.started_at, pipeline.finished_at)],
  ];
  if (pipeline.branch) values.splice(2, 0, [t("Branch"), pipeline.branch]);
  if (pipeline.category) values.push([t("Category"), pipelineCategory(pipeline)]);
  if (pipeline.pipeline_name) values.push([t("Pipeline"), pipeline.pipeline_name], [t("Runner OS"), osLabel(pipeline.runner_os)]);
  values.push([t("Sparse View"), pipeline.sparse_view_name || t("dynamic.noSparseView")]);
  for (const [label, value] of values) {
    const item = document.createElement("div"); item.className = "summary-item";
    const title = document.createElement("span"); title.textContent = label;
    const content = document.createElement("strong"); content.textContent = value;
    item.append(title, content); elements["detail-summary"].append(item);
  }
  if (pipeline.error) {
    const item = document.createElement("div"); item.className = "summary-item summary-item--error";
    const title = document.createElement("span"); title.textContent = t("Failure reason");
    const content = document.createElement("strong"); content.textContent = pipeline.error;
    item.append(title, content); elements["detail-summary"].append(item);
  }
  if (pipeline.sparse_view_name && sparseViewRules) {
    const item = document.createElement("div"); item.className = "summary-item summary-item--view-rules";
    const title = document.createElement("span"); title.textContent = `${t("View rules")} · ${t("dynamic.viewSnapshot")}`;
    const content = document.createElement("code"); content.textContent = sparseViewRules.trim();
    item.append(title, content); elements["detail-summary"].append(item);
  }
}

function renderJobs(jobs) {
  elements["job-list"].replaceChildren();
  elements["job-count"].textContent = tc("dynamic.jobs", jobs.length);
  if (!jobs.length) {
    const empty = textNode(t("dynamic.workerPreparing"), "job-empty");
    elements["job-list"].append(empty);
    return;
  }
  for (const job of jobs) {
    const row = document.createElement("div"); row.className = "job-row";
    const stateIcon = textNode(job.status === "succeeded" ? "✓" : job.status === "failed" ? "!" : job.status === "running" ? "▶" : "·", `job-state job-state--${job.status}`);
    row.append(stateIcon, textNode(job.name, "job-name"), textNode(job.stage, "job-stage"), textNode(duration(job.started_at, job.finished_at), "job-duration"));
    elements["job-list"].append(row);
  }
}

function renderLogs(logs, prunedAt) {
  const terminal = elements["pipeline-log"];
  const scroll = terminal.scrollTop;
  const cache = state.detailLogView;
  terminal.replaceChildren();
  if (prunedAt) terminal.append(textNode(t("dynamic.logsPruned"), "terminal-muted"));
  if (!logs.length && !prunedAt) terminal.append(textNode(t(cache?.busy ? "dynamic.loadingLogs" : "dynamic.noLogs"), "terminal-muted"));
  for (const log of logs) {
    const line = document.createElement("span");
    line.className = log.stream === "stderr" ? "terminal-stderr" : log.stream === "system" ? "terminal-system" : "";
    line.textContent = log.content;
    terminal.append(line);
  }
  terminal.scrollTop = cache?.follow ? terminal.scrollHeight : scroll;
  const more = elements["detail-log-more"];
  more.hidden = !cache || (!cache.more && !cache.error);
  more.disabled = !cache || cache.busy;
  more.textContent = dlt(cache?.error ? "retry" : "more");
  const follow = elements["detail-log-follow"];
  follow.disabled = !cache;
  follow.textContent = dlt("follow");
  follow.setAttribute("aria-pressed", String(Boolean(cache?.follow)));
  elements["detail-log-restart"].disabled = !cache || cache.busy;
  elements["detail-log-restart"].textContent = dlt("restart");
  elements["detail-log-note"].textContent = cache ? [cache.error, cache.trimmed && dlt("trimmed"), !cache.follow && dlt("paused"), cache.more && dlt("pending")].filter(Boolean).join(" ") : "";
}

async function cancelPipeline() {
  if (!state.selectedId) return;
  elements["cancel-pipeline-button"].disabled = true;
  try {
    await api(`/api/v1/pipelines/${encodeURIComponent(state.selectedId)}/cancel`, { method: "POST", headers: { "X-CSRF-Token": csrfToken() } });
    toast(t("dynamic.cancelRequested"), "success");
    await loadPipelines(false);
    await loadPipelineDetail(state.selectedId);
  } catch (error) {
    elements["cancel-pipeline-button"].disabled = false;
    toast(error.message, "error");
  }
}

async function logout() {
  try {
    await api("/auth/logout", { method: "POST", headers: { "X-CSRF-Token": csrfToken() } });
    window.location.reload();
  } catch (error) { toast(error.message, "error"); }
}

async function refreshActiveViews() {
  if (!document.hidden && state.section === "operations") {
    if (Date.now() - operations.lastAttempt >= 30000) await loadOperations();
    if (state.selectedId && elements["pipeline-detail-dialog"].open) {
      try { await loadPipelineDetail(state.selectedId); } catch (_) { /* next poll retries */ }
    }
    return;
  }
  if (document.hidden || isManagement() || state.section === "repository-tree") return;
  if (state.section === "overview" && Date.now() - workspaceOverview.lastAttempt >= 30000) await loadOverview();
  if (state.section === "repository-links") {
    if (!state.repositoryLinksBusy && state.repositoryLinkOperations.some(item => item.status === "running")) await loadRepositoryLinkOperations();
    return;
  }
  if (state.section === "runners") {
    await loadRunners(false);
    return;
  }
  if (state.section === "repositories" && !elements["pipeline-detail-dialog"].open) return;
  if (!state.pipelineHasOlderPages && !state.pipelineSearchTimer) await loadPipelines(false);
  if (state.section === "graphs") {
    await loadPipelineGraphs(false);
  }
  if (state.selectedId && elements["pipeline-detail-dialog"].open) {
    try { await loadPipelineDetail(state.selectedId); } catch (_) { /* next poll retries */ }
  }
}

function csrfToken() {
  const cookie = document.cookie.split("; ").find((entry) => entry.startsWith("lorehub_csrf="));
  return cookie ? decodeURIComponent(cookie.slice("lorehub_csrf=".length)) : "";
}

function repositoryName(url) {
  const normalized = url.replace(/\/+$/, "");
  return normalized.slice(normalized.lastIndexOf("/") + 1) || normalized;
}
function initials(name) { return name.split(/\s+/).filter(Boolean).slice(0, 2).map((part) => part[0]).join("").toUpperCase(); }
function statusLabel(status) {
  const key = `status.${status}`;
  const label = t(key);
  return label === key ? status : label;
}
function localeTag() { return state.locale === "ko" ? "ko-KR" : state.locale === "zh-CN" ? "zh-CN" : "en-US"; }
function formatClock(date) { return new Intl.DateTimeFormat(localeTag(), { hour: "2-digit", minute: "2-digit" }).format(date); }
function formatDate(value) { return new Intl.DateTimeFormat(localeTag(), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)); }
function relativeTime(value) {
  const seconds = Math.round((new Date(value).getTime() - Date.now()) / 1000);
  const abs = Math.abs(seconds);
  const formatter = new Intl.RelativeTimeFormat(localeTag(), { numeric: "auto" });
  if (abs < 60) return formatter.format(seconds, "second");
  if (abs < 3600) return formatter.format(Math.round(seconds / 60), "minute");
  if (abs < 86400) return formatter.format(Math.round(seconds / 3600), "hour");
  return formatter.format(Math.round(seconds / 86400), "day");
}
function duration(start, finish) {
  if (!start) return "—";
  const milliseconds = Math.max(0, new Date(finish || Date.now()).getTime() - new Date(start).getTime());
  const seconds = Math.floor(milliseconds / 1000);
  if (seconds < 60) return t("unit.second", { count: seconds });
  const minutes = Math.floor(seconds / 60);
  return t("unit.minuteSecond", { minutes, seconds: seconds % 60 });
}
function toast(message, kind = "info") {
  const item = document.createElement("div");
  item.className = `toast toast--${kind}`;
  item.setAttribute("role", "status");
  item.textContent = message;
  elements["toast-region"].append(item);
  window.setTimeout(() => item.remove(), 4500);
}
