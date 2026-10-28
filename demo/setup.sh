#!/usr/bin/env bash
# PROTOTYPE: builds the stakeholder demo world under ${TMPDIR}cellworks-demo.
# Idempotent: wipes the folder first. Needs the network once for npm install
# (cached in demo/.cache afterwards).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="${TMPDIR:-/tmp}"
ROOT="${ROOT%/}/cellworks-demo"
rm -rf "$ROOT"
mkdir -p "$ROOT/projects" "$ROOT/jira" "$ROOT/cases" "$ROOT/case-snapshots" "$ROOT/transcripts"
ROOT="$(cd "$ROOT" && pwd -P)"

# ---- shopfront
SHOP="$ROOT/projects/shopfront"
mkdir -p "$SHOP"
tar -C "$HERE/shopfront" --exclude node_modules --exclude .cellworks -cf - . | tar -C "$SHOP" -xf -

cd "$SHOP"
git init -q -b main
git config user.name "Dana Weber"
git config user.email "dana@shopfront.example"

n=0
commit() { # message, paths...
  local msg="$1"; shift
  git add -- "$@"
  n=$((n + 1))
  # one day apart, so the history has a shape
  local when="2026-09-$(printf '%02d' $((n + 8)))T10:00:00"
  GIT_AUTHOR_DATE="$when" GIT_COMMITTER_DATE="$when" git commit -q -m "$msg"
}

commit $'chore: project scaffold\n\nRefs: STORY-001' package.json tsconfig.json vite.config.ts index.html .gitignore README.md
commit $'feat(core): money, ids, events and http helpers\n\nRefs: STORY-002' src/core
commit $'feat(catalog): product search\n\nRefs: STORY-012' src/catalog/product.ts src/catalog/price.ts src/catalog/search.ts src/catalog/search.test.ts
commit $'feat(cart): cart and totals\n\nRefs: STORY-017' src/cart
commit $'feat(checkout): place an order and pay\n\nRefs: STORY-020' src/checkout
commit $'feat(server): hexagonal orders backend\n\nRefs: STORY-022' server
commit $'feat(ui): catalog, cart and checkout views\n\nRefs: STORY-024' src/ui
commit $'docs(domain): bounded contexts and context map\n\nRefs: STORY-025' docs/domain
commit $'docs(adr): architecture decisions\n\nRefs: STORY-025' docs/arc42 docs/adr sdlc
commit $'ci: sdlc pipeline and dev server launch config\n\nRefs: STORY-030' .github .claude
commit $'feat(infra): terraform, kubernetes and Dockerfile\n\nRefs: STORY-031' infra Dockerfile
if [ -n "$(git status --porcelain)" ]; then
  echo "setup: files left uncommitted:" >&2; git status --short >&2; exit 1
fi

# ---- decoys
mkdir -p "$ROOT/projects/billing-api/src" "$ROOT/projects/ops-runbooks"
cd "$ROOT/projects/billing-api"
cat > Cargo.toml <<'T'
[package]
name = "billing-api"
version = "0.1.0"
edition = "2021"
T
cat > src/main.rs <<'T'
mod invoice;
mod tax;

fn main() {
    let invoice = invoice::Invoice::new("INV-1", 12_000);
    println!("{} {}", invoice.id, tax::with_vat(invoice.cents));
}
T
cat > src/invoice.rs <<'T'
pub struct Invoice {
    pub id: String,
    pub cents: u64,
}

impl Invoice {
    pub fn new(id: &str, cents: u64) -> Invoice {
        Invoice { id: id.to_owned(), cents }
    }
}
T
cat > src/tax.rs <<'T'
pub fn with_vat(cents: u64) -> u64 {
    cents + cents * 19 / 100
}
T
printf 'target\n' > .gitignore
git init -q -b main && git config user.name "Sam Ortiz" && git config user.email "sam@billing.example"
git add . && git commit -q -m "feat: invoice and vat"

cd "$ROOT/projects/ops-runbooks"
printf '# Runbooks\n\nHow we run things in production.\n' > README.md
printf '# Restart the checkout pod\n\n1. kubectl rollout restart deploy/checkout\n2. Watch the rollout\n' > restart-checkout.md
printf '# Rotate the payment secret\n\n1. Write the new key to Vault\n2. Restart the pods\n' > rotate-payment-secret.md
git init -q -b main && git config user.name "Sam Ortiz" && git config user.email "sam@ops.example"
git add . && git commit -q -m "docs: first runbooks"

# ---- history worktree, detached at the tip
git -C "$SHOP" worktree add -q --detach "$ROOT/projects/shopfront-history" HEAD

# ---- jira, cases, agents, settings
cp "$HERE"/jira/*.json "$ROOT/jira/"
for f in "$HERE"/cases/*.json; do
  sed "s#__ROOT__#$ROOT#g" "$f" > "$ROOT/case-snapshots/$(basename "$f")"
done
echo '[]' > "$ROOT/agents.json"
cat > "$ROOT/settings.toml" <<T
[projects."$SHOP"]
contexts.Catalog = ["src/catalog"]
contexts.Cart = ["src/cart"]
contexts.Checkout = ["src/checkout"]

[hosts."staging"]
projects = ["/srv/shopfront"]
T

# ---- dependencies, cached
cd "$SHOP"
export npm_config_cache="${CELLWORKS_NPM_CACHE:-$HERE/.cache/npm}"
if ! npm install --prefer-offline --no-audit --no-fund --no-package-lock >"$ROOT/npm-install.log" 2>&1; then
  echo "setup: WARNING npm install failed (offline and not cached?). The dev server will not start. See $ROOT/npm-install.log" >&2
fi

echo "demo world ready: $ROOT"
