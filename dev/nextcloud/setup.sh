#!/usr/bin/env bash
# Starts the test Nextcloud, installs the Calendar and Tasks apps, makes a test user with a task calendar and an
# event calendar, and prints working credentials. Safe to run again.
#
#   dev/nextcloud/setup.sh            # http://localhost:8080
#   NC_PORT=8081 dev/nextcloud/setup.sh
#   docker compose -f dev/nextcloud/docker-compose.yml down -v    # throw it away
set -euo pipefail
cd "$(dirname "$0")"

export NC_PORT="${NC_PORT:-8080}"
# passwords of this throwaway instance are made once and kept in `.env` (ignored by git)
if [[ ! -f .env ]]; then
  gen() { head -c 24 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 20; }
  printf 'NC_DB_PASSWORD=%s\nNC_ADMIN_PASSWORD=%s\nNC_TEST_PASSWORD=%s\n' "$(gen)" "$(gen)" "$(gen)" > .env
fi
set -a; source .env; set +a

docker compose up -d
echo "waiting for the install (the first start takes a minute or two)..."
occ() { docker compose exec -T -u www-data app php occ "$@"; }
installed() { [[ "$(occ status 2>/dev/null || true)" == *"installed: true"* ]]; }
for _ in $(seq 1 120); do
  installed && break
  sleep 3
done
installed || { echo "Nextcloud did not come up" >&2; exit 1; }

occ app:install calendar >/dev/null 2>&1 || occ app:enable calendar >/dev/null
occ app:install tasks >/dev/null 2>&1 || occ app:enable tasks >/dev/null

USER=notatnik
if ! occ user:info "$USER" >/dev/null 2>&1; then
  docker compose exec -T -u www-data -e OC_PASS="$NC_TEST_PASSWORD" app php occ user:add --password-from-env --display-name="Notatnik test" "$USER" >/dev/null
fi
occ dav:create-calendar "$USER" tasks-test >/dev/null 2>&1 || true
occ dav:create-calendar "$USER" events-test >/dev/null 2>&1 || true
APP_PASSWORD=$(occ user:auth-tokens:add "$USER" --name notatnik-tests 2>/dev/null | tail -1 || true)

echo
echo "Nextcloud:     http://localhost:$NC_PORT"
echo "User:          $USER"
echo "Password:      $NC_TEST_PASSWORD"
[[ -n "$APP_PASSWORD" ]] && echo "App password:  $APP_PASSWORD"
echo "Calendars:     tasks-test, events-test (and the default 'personal')"
echo
echo "NC_SERVER=http://localhost:$NC_PORT NC_USER=$USER NC_PASSWORD=<password> NC_CALENDAR=tasks-test cargo run -p notatnik-caldav --example live"
