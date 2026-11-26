#!/usr/bin/env bash
# E-133 evidence, and nothing else: log what the GitHub API says this run's Pages artifact is, so the next
# `deploy-pages` "Found 0 artifact(s)" produces data instead of a third theory.
#
# READ-ONLY, and it can never fail the job: it makes two GET requests, prints what they return, and exits 0 whatever
# happens (no `gh`, no token, a 403, a timeout, unparsable JSON). It is not a fix and does not attempt one; the
# deploy's outcome is exactly what it was without this step.
#
# Usage: pages-artifact-probe.sh <label>      (label: "before-deploy" / "after-deploy")
# Reads:  GITHUB_RUN_ID GITHUB_RUN_ATTEMPT GITHUB_EVENT_NAME GITHUB_SHA GITHUB_REPOSITORY (set by Actions),
#         GH_TOKEN, and UPLOAD_ARTIFACT_ID (what upload-pages-artifact says it finalised; may be empty).
set +e
label="${1:-probe}"
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

echo "== pages-artifact-probe [$label] $(now)"
echo "run=${GITHUB_RUN_ID:-?} attempt=${GITHUB_RUN_ATTEMPT:-?} event=${GITHUB_EVENT_NAME:-?} sha=${GITHUB_SHA:-?}"
echo "upload-pages-artifact reported artifact_id=${UPLOAD_ARTIFACT_ID:-<none>}"

# One line per artifact: the fields that would tell a store race from a mis-association from an expiry.
fmt='"  id=\(.id) name=\(.name) size=\(.size_in_bytes) expired=\(.expired) created=\(.created_at) updated=\(.updated_at) run=\(.workflow_run.id // "none") digest=\(.digest // "none")"'

echo "-- this run's artifacts (GET /actions/runs/{run}/artifacts):"
out="$(timeout 60 gh api "repos/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}/artifacts" \
        --jq "\"  total_count=\\(.total_count)\", (.artifacts[] | $fmt)" 2>&1)"
rc=$?
echo "${out:-  <empty>}"
echo "  (gh exit status: $rc)"

echo "-- the repository's newest 'github-pages' artifacts (GET /actions/artifacts?name=github-pages):"
out="$(timeout 60 gh api "repos/${GITHUB_REPOSITORY}/actions/artifacts?name=github-pages&per_page=5" \
        --jq "\"  total_count=\\(.total_count)\", (.artifacts[] | $fmt)" 2>&1)"
rc=$?
echo "${out:-  <empty>}"
echo "  (gh exit status: $rc)"
echo "== end pages-artifact-probe [$label]"
exit 0
