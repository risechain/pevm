#!/usr/bin/env bash
# Downloads and extracts the stable Ethereum Execution Spec Test (EEST) fixtures
# into crates/pevm/tests/ethereum/fixtures/. Run this once before running tests:
#
#   bash scripts/fetch-eest-fixtures.sh
#
# To pin to a specific release, set EEST_VERSION before running:
#
#   EEST_VERSION=v5.4.0 bash scripts/fetch-eest-fixtures.sh

set -euo pipefail

EEST_VERSION="${EEST_VERSION:-$(
    curl -sf https://api.github.com/repos/ethereum/execution-spec-tests/releases/latest \
        | python3 -c "import sys,json; print(json.load(sys.stdin)['tag_name'])"
)}"
DEST="crates/pevm/tests/ethereum/fixtures"
URL="https://github.com/ethereum/execution-spec-tests/releases/download/${EEST_VERSION}/fixtures_stable.tar.gz"

echo "Fetching EEST ${EEST_VERSION} -> ${DEST}/"
mkdir -p "${DEST}"
curl -fL "${URL}" | tar -xz --strip-components=1 -C "${DEST}"
echo "Done."
