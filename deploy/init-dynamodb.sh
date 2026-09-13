#!/bin/sh
set -eu

aws_local() {
    aws --endpoint-url "$DYNAMODB_ENDPOINT" dynamodb "$@"
}

attempt=0
until aws_local list-tables >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 30 ]; then
        echo "DynamoDB Local did not become ready" >&2
        exit 1
    fi
    sleep 1
done

create_table() {
    table_name=$1
    shift
    if ! aws_local describe-table --table-name "$table_name" >/dev/null 2>&1; then
        aws_local create-table --table-name "$table_name" --billing-mode PAY_PER_REQUEST "$@" >/dev/null
    fi
}

create_table "$DYNAMODB_FRAGMENTS_TABLE" \
    --attribute-definitions AttributeName=hash,AttributeType=B AttributeName=repository_context,AttributeType=B \
    --key-schema AttributeName=hash,KeyType=HASH AttributeName=repository_context,KeyType=RANGE

create_table "$DYNAMODB_FRAGMENT_STATE_TABLE" \
    --attribute-definitions AttributeName=hash,AttributeType=B \
    --key-schema AttributeName=hash,KeyType=HASH

create_table "$DYNAMODB_MUTABLE_TABLE" \
    --attribute-definitions AttributeName=repository_id,AttributeType=B AttributeName=key,AttributeType=B \
    --key-schema AttributeName=repository_id,KeyType=HASH AttributeName=key,KeyType=RANGE

create_table "$DYNAMODB_LOCKS_TABLE" \
    --attribute-definitions AttributeName=hash,AttributeType=B AttributeName=repositoryBranch,AttributeType=B AttributeName=ownerId,AttributeType=S AttributeName=repository,AttributeType=B AttributeName=branch,AttributeType=B AttributeName=description,AttributeType=S \
    --key-schema AttributeName=hash,KeyType=HASH AttributeName=repositoryBranch,KeyType=RANGE \
    --global-secondary-indexes \
      'IndexName=owner-repo-branch,KeySchema=[{AttributeName=ownerId,KeyType=HASH},{AttributeName=repositoryBranch,KeyType=RANGE}],Projection={ProjectionType=ALL}' \
      'IndexName=repo-branch,KeySchema=[{AttributeName=repository,KeyType=HASH},{AttributeName=branch,KeyType=RANGE}],Projection={ProjectionType=ALL}' \
      'IndexName=repo-branch-description,KeySchema=[{AttributeName=repositoryBranch,KeyType=HASH},{AttributeName=description,KeyType=RANGE}],Projection={ProjectionType=ALL}'

for table_name in \
    "$DYNAMODB_FRAGMENTS_TABLE" \
    "$DYNAMODB_FRAGMENT_STATE_TABLE" \
    "$DYNAMODB_MUTABLE_TABLE" \
    "$DYNAMODB_LOCKS_TABLE"
do
    aws_local wait table-exists --table-name "$table_name"
done
