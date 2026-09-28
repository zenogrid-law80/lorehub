#!/bin/sh
# An isolated Lore boundary fixture; state lives next to the copied executable.
set -eu
work=$(dirname "$0")
shift 3
if [ "${1:-}" = --identity-token ]; then shift 4; fi
printf '%s\n' "$*" >> "$work/commands"
revision=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
case "${1:-}:${2:-}" in
  repository:list)
    printf '%s\n' '{"tagName":"repositoryListEntry","data":{"id":"11111111111111111111111111111111","name":"root"}}'
    ;;
  repository:clone) mkdir -p "$6" ;;
  --repository:repository)
    printf '%s\n' "{\"tagName\":\"branchListEntry\",\"data\":{\"id\":\"release-id\",\"name\":\"release\",\"location\":\"remote\",\"latest\":\"$revision\"}}"
    if [ -f "$work/published" ]; then
      if [ -f "$work/published-pins" ]; then revision=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb; fi
      printf '%s\n' "{\"tagName\":\"branchListEntry\",\"data\":{\"id\":\"feature-id\",\"name\":\"feature/release\",\"location\":\"remote\",\"latest\":\"$revision\"}}"
    fi
    ;;
  clone:--bare)
    [ "$3" = --branch ]
    [ "$4" = release-id ]
    [ "$5" = -- ]
    mkdir -p "$7"
    ;;
  branch:switch)
    [ "$3" = --bare ]
    [ "$4" = -- ]
    case "$5" in release-id|feature-id) ;; *) exit 98;; esac
    if [ "$#" -gt 5 ]; then
      case "$6" in aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) ;; *) exit 98;; esac
      printf '%s' "$6" > .selected-revision
    fi
    printf '%s' "$5" > .selected-branch
    ;;
  clone:--revision)
    case "$3" in aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) ;; *) exit 98;; esac
    mkdir -p "$6"
    # A shared revision alone gives the original branch, as Lore does.
    printf '%s' release-id > "$6/.selected-branch"
    ;;
  --repository:.)
    [ "$3 $4 $5" = '--remote link list' ]
    branch=$(cat .selected-branch)
    tracking=true
    if [ -f "$work/explicit-pins" ]; then
      tracking=false
      branch=release-id
      if [ "$(cat .selected-revision)" = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ]; then branch=feature-id; fi
    fi
    if [ -f "$work/wrong-link" ]; then branch=release-id; tracking=false; fi
    printf '%s\n' "{\"tagName\":\"linkEntry\",\"data\":{\"link\":\"22222222222222222222222222222222\",\"linkPath\":\"Source\",\"sourcePath\":\".\",\"branch\":\"$branch\",\"tracking\":$tracking,\"revision\":\"$revision\"}}"
    printf '%s\n' "{\"tagName\":\"linkEntry\",\"data\":{\"link\":\"33333333333333333333333333333333\",\"linkPath\":\"Fixed\",\"sourcePath\":\".\",\"branch\":\"fixed-id\",\"tracking\":false,\"revision\":\"$revision\"}}"
    ;;
  --local:branch)
    if [ -f "$work/advanced" ]; then revision=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb; fi
    printf '%s\n' "{\"tagName\":\"branchListEntry\",\"data\":{\"id\":\"release-id\",\"name\":\"release\",\"location\":\"local\",\"isCurrent\":true,\"latest\":\"$revision\"}}"
    ;;
  branch:create)
    [ "$3" = -- ]
    [ "$4" = feature/release ]
    if [ -f "$work/denied" ]; then
      printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"Source repository permission denied"}}}'
      exit 1
    fi
    touch "$work/source-created"
    printf '%s' feature-id > .selected-branch
    printf '%s\n' '{"tagName":"linkBranchCreate","data":{"linkPath":"Source","branch":"feature-id"}}'
    ;;
  push:|push:--)
    [ "$(cat .selected-branch)" = feature-id ]
    [ -f "$work/source-created" ]
    if [ -f "$work/fail-push" ]; then
      printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"Root push failed"}}}'
      exit 1
    fi
    touch "$work/published"
    # Lore's unnamed push uses the parent checkout anchor. Only a named push
    # publishes the branch head containing the newly serialized explicit pins.
    if [ "${2:-}" = -- ]; then
      [ "$3" = feature/release ]
      if [ -f "$work/explicit-pins" ]; then touch "$work/published-pins"; fi
    fi
    printf '%s\n' '{"tagName":"branchPushRevisionPushEnd","data":{"newRemoteRevision":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}'
    ;;
  link:update)
    [ "$3 $4" = '-- Source' ]
    [ "$(cat .selected-branch)" = feature-id ]
    printf '%s\n' '{"tagName":"linkChange","data":{"revision":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}'
    ;;
  commit:*)
    [ "$(cat .selected-branch)" = feature-id ]
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{"tagName":"complete","data":{"status":0}}'
