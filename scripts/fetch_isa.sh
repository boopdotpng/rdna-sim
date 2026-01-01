#!/bin/bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
echo "Fetching ISA XMLs..."
out_dir="${ROOT_DIR}/data"
tmp_dir="$(mktemp -d)"
zip_path="${tmp_dir}/isa.zip"

cleanup_fetch() {
  rm -rf "${tmp_dir}"
}
trap cleanup_fetch EXIT

mkdir -p "${out_dir}"
curl -L "https://gpuopen.com/download/machine-readable-isa/latest/" -o "${zip_path}"
unzip -o "${zip_path}" -d "${tmp_dir}"
find "${tmp_dir}" -type f -name "*.xml" -exec cp -f {} "${out_dir}/" \;
echo "Removing ISA XMLs for rdna1/rdna2/cdna1/cdna2..."
rm -f \
  "${out_dir}/amdgpu_isa_cdna1.xml" \
  "${out_dir}/amdgpu_isa_cdna2.xml" \
  "${out_dir}/amdgpu_isa_rdna1.xml" \
  "${out_dir}/amdgpu_isa_rdna2.xml"

echo "Downloaded AMDGPU ISA files to ${out_dir}"
