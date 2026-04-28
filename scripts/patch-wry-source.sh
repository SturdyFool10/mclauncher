#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CARGO_HOME_DIR="${CARGO_HOME:-${HOME}/.cargo}"
WRY_VERSION="$(
  awk '
    $0 == "name = \"wry\"" {
      in_wry = 1
      next
    }
    in_wry && $0 ~ /^version = / {
      gsub(/"/, "", $3)
      print $3
      exit
    }
    /^\[\[package\]\]/ {
      in_wry = 0
    }
  ' "${REPO_ROOT}/Cargo.lock"
)"

if [[ -z "${WRY_VERSION}" ]]; then
  echo "Could not resolve wry version from Cargo.lock." >&2
  exit 1
fi

find_wry_source_dir() {
  local registry_src="${CARGO_HOME_DIR}/registry/src"

  if [[ ! -d "${registry_src}" ]]; then
    return 0
  fi

  find "${registry_src}" -maxdepth 2 -type d -name "wry-${WRY_VERSION}" | head -n 1
}

unpack_wry_crate() {
  local crate_file
  local registry_name
  local source_parent
  local source_dir

  crate_file="$(find "${CARGO_HOME_DIR}/registry/cache" -type f -name "wry-${WRY_VERSION}.crate" 2>/dev/null | head -n 1)"
  if [[ -z "${crate_file}" ]]; then
    return 1
  fi

  registry_name="$(basename -- "$(dirname -- "${crate_file}")")"
  source_parent="${CARGO_HOME_DIR}/registry/src/${registry_name}"
  source_dir="${source_parent}/wry-${WRY_VERSION}"

  mkdir -p "${source_parent}"
  if [[ ! -d "${source_dir}" ]]; then
    tar -xzf "${crate_file}" -C "${source_parent}"
  fi
}

wry_source_dir="$(find_wry_source_dir)"

if [[ -z "${wry_source_dir}" ]]; then
  unpack_wry_crate || true
  wry_source_dir="$(find_wry_source_dir)"
fi

if [[ -z "${wry_source_dir}" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to fetch the cached wry source." >&2
    exit 1
  fi

  echo "[wry-patch] wry ${WRY_VERSION} not cached yet; fetching crate sources..."
  cargo fetch --locked --manifest-path "${REPO_ROOT}/crates/vertexlauncher/Cargo.toml" >/dev/null
  unpack_wry_crate || true
  wry_source_dir="$(find_wry_source_dir)"
fi

if [[ -z "${wry_source_dir}" ]]; then
  echo "Could not find cached wry source for version ${WRY_VERSION} under ${CARGO_HOME_DIR}/registry/src." >&2
  exit 1
fi

target_file="${wry_source_dir}/src/webview/webkitgtk/mod.rs"
if [[ ! -f "${target_file}" ]]; then
  echo "Missing expected wry source file: ${target_file}" >&2
  exit 1
fi

needle='use webkit2gtk::traits::SettingsExt;'
anchor='use webkit2gtk_sys::{'

if grep -Fqx "${needle}" "${target_file}"; then
  if grep -F "SettingsExt," "${target_file}" >/dev/null; then
    target_dir="$(dirname -- "${target_file}")"
    tmp_file="$(mktemp "${target_dir}/mod.rs.XXXXXX")"
    grep -Fxv "${needle}" "${target_file}" > "${tmp_file}"
    mv "${tmp_file}" "${target_file}"
    echo "[wry-patch] removed obsolete duplicate SettingsExt import: ${target_file}"
    exit 0
  fi

  echo "[wry-patch] already patched: ${target_file}"
  exit 0
fi

if grep -F "SettingsExt," "${target_file}" >/dev/null; then
  echo "[wry-patch] not needed for wry ${WRY_VERSION}: ${target_file}"
  exit 0
fi

target_dir="$(dirname -- "${target_file}")"
tmp_file="$(mktemp "${target_dir}/mod.rs.XXXXXX")"
cleanup() {
  rm -f "${tmp_file}"
}
trap cleanup EXIT

if ! awk -v needle="${needle}" -v anchor="${anchor}" '
  $0 == anchor {
    print needle
    patched = 1
  }
  { print }
  END {
    if (!patched) {
      exit 1
    }
  }
' "${target_file}" > "${tmp_file}"; then
  echo "[wry-patch] failed: could not find anchor in ${target_file}" >&2
  exit 1
fi

mv "${tmp_file}" "${target_file}"
trap - EXIT
echo "[wry-patch] patched: ${target_file}"
