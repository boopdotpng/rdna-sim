#!/usr/bin/env python
import argparse
import os
import xml.etree.ElementTree as ET
from typing import Dict, List, Tuple

MAX_LINE = 150


def rust_string_literal(value: str) -> str:
  out = ['"']
  for ch in value:
    code = ord(ch)
    if ch == "\\":
      out.append("\\\\")
    elif ch == '"':
      out.append('\\"')
    elif ch == "\n":
      out.append("\\n")
    elif ch == "\r":
      out.append("\\r")
    elif ch == "\t":
      out.append("\\t")
    elif 32 <= code <= 126:
      out.append(ch)
    else:
      out.append(f"\\u{{{code:x}}}")
  out.append('"')
  return "".join(out)


def wrap_rust_string(value: str, indent: int, prefix_len: int = 0) -> str:
  literal = rust_string_literal(value)
  if len(literal) + indent + prefix_len <= MAX_LINE:
    return literal
  inner = literal[1:-1]
  chunk_size = 80
  chunks = [inner[i:i + chunk_size] for i in range(0, len(inner), chunk_size)]
  lines = ["concat!("]
  for chunk in chunks:
    lines.append(" " * (indent + 2) + rust_string_literal(chunk) + ",")
  lines.append(" " * indent + ")")
  return "\n".join(lines)


def format_str_list(items: List[str], indent: int, prefix_len: int = 0) -> str:
  if not items:
    return "&[]"
  rendered = [rust_string_literal(item) for item in items]
  inline = ", ".join(rendered)
  if len(inline) + indent + prefix_len + 3 <= MAX_LINE:
    return f"&[{inline}]"
  lines = ["&["]
  for item in rendered:
    lines.append(" " * (indent + 2) + f"{item},")
  lines.append(" " * indent + "]")
  return "\n".join(lines)


def to_variant(name: str) -> str:
  parts = [p for p in name.split("_") if p]
  out_parts = []
  for part in parts:
    lower = part.lower()
    if lower and lower[0].isdigit():
      out_parts.append("N" + lower)
    else:
      out_parts.append(lower[:1].upper() + lower[1:])
  variant = "".join(out_parts)
  if variant and variant[0].isdigit():
    variant = "N" + variant
  return variant


def unique_variants(names: List[str]) -> Dict[str, str]:
  seen: Dict[str, int] = {}
  mapping: Dict[str, str] = {}
  for name in names:
    base = to_variant(name)
    count = seen.get(base, 0)
    seen[base] = count + 1
    if count == 0:
      mapping[name] = base
    else:
      mapping[name] = f"{base}_{count + 1}"
  return mapping


def render_instruction_def(inst: dict, variant: str) -> str:
  indent = 4
  lines = ["  InstructionDef {", f"    name: {rust_string_literal(inst['normalized_name'])},"]
  lines.append(f"    instruction: Instruction::{variant},")
  lines.append(f"    args: {format_arg_specs(inst.get('operands', []), indent + 4, len('args: '))},")
  lines.append(
    f"    encodings: {format_str_list(inst.get('available_encodings', []), indent + 4, len('encodings: '))},"
  )
  lines.append("  },")
  return "\n".join(lines)


def format_arg_specs(operands: List[dict], indent: int, prefix_len: int = 0) -> str:
  if not operands:
    return "&[]"
  rendered = [render_arg_spec(operand) for operand in operands]
  inline = ", ".join(rendered)
  if len(inline) + indent + prefix_len + 3 <= MAX_LINE:
    return f"&[{inline}]"
  lines = ["&["]
  for item in rendered:
    lines.append(" " * (indent + 2) + f"{item},")
  lines.append(" " * indent + "]")
  return "\n".join(lines)


def render_arg_spec(operand: dict) -> str:
  kind = operand_kind(operand.get("operand_type", ""))
  if kind in {"Imm", "RegOrImm"}:
    data_type = data_type_variant(operand.get("data_format") or "")
  else:
    data_type = "None"
  size_text = operand.get("size") or ""
  try:
    width = int(size_text)
  except ValueError:
    width = 0
  return (
    "ArgSpec { "
    f"kind: ArgKind::{kind}, "
    f"data_type: DataType::{data_type}, "
    f"width: {width} "
    "}"
  )


def data_type_variant(data_format: str) -> str:
  if not data_format:
    return "Unknown"
  name = data_format.upper()
  mapping = {
    "FMT_ANY": "Any",
    "FMT_NUM_B1": "B1",
    "FMT_NUM_B8": "B8",
    "FMT_NUM_B16": "B16",
    "FMT_NUM_B32": "B32",
    "FMT_NUM_B64": "B64",
    "FMT_NUM_B96": "B96",
    "FMT_NUM_B128": "B128",
    "FMT_NUM_F16": "F16",
    "FMT_NUM_F32": "F32",
    "FMT_NUM_F64": "F64",
    "FMT_NUM_BF16": "BF16",
    "FMT_NUM_I8": "I8",
    "FMT_NUM_I16": "I16",
    "FMT_NUM_I24": "I24",
    "FMT_NUM_I32": "I32",
    "FMT_NUM_I64": "I64",
    "FMT_NUM_U8": "U8",
    "FMT_NUM_U16": "U16",
    "FMT_NUM_U24": "U24",
    "FMT_NUM_U32": "U32",
    "FMT_NUM_U64": "U64",
    "FMT_NUM_M64": "M64",
    "FMT_NUM_PK2_B16": "Pk2B16",
    "FMT_NUM_PK2_BF16": "Pk2BF16",
    "FMT_NUM_PK2_F16": "Pk2F16",
    "FMT_NUM_PK2_I16": "Pk2I16",
    "FMT_NUM_PK2_U16": "Pk2U16",
    "FMT_NUM_PK2_U8": "Pk2U8",
    "FMT_NUM_PK4_B8": "Pk4B8",
    "FMT_NUM_PK4_IU8": "Pk4IU8",
    "FMT_NUM_PK4_U8": "Pk4U8",
    "FMT_NUM_PK8_IU4": "Pk8IU4",
    "FMT_NUM_PK8_U4": "Pk8U4",
  }
  return mapping.get(name, "Unknown")


def render_instruction_pair(name: str, variant: str) -> str:
  return f"  ({rust_string_literal(name)}, Instruction::{variant}),"


def parse_csv(value: str) -> List[str]:
  return [item.strip() for item in value.split(",") if item.strip()]


def normalize_name(name: str) -> str:
  return name.lower()


def operand_kind(operand_type: str) -> str:
  operand = operand_type.upper()
  if "VGPR_OR_INLINE" in operand:
    return "RegOrImm"
  if "VGPR" in operand:
    return "Vgpr"
  if operand in {
    "OPR_EXEC",
    "OPR_VCC",
    "OPR_PC",
    "OPR_SDST_NULL",
    "OPR_SDST_EXEC",
    "OPR_SDST_M0",
    "OPR_SSRC_SPECIAL_SCC",
    "OPR_SSRC_LANESEL",
    "OPR_SREG_M0_INL",
    "OPR_HWREG",
  }:
    return "Special"
  if any(token in operand for token in ["SGPR", "SREG", "SSRC", "SDST", "SCC", "M0"]):
    return "Sgpr"
  if any(token in operand for token in ["SIMM", "LIT", "INLINE", "IMM", "SENDMSG", "VERSION"]):
    return "Imm"
  if any(token in operand for token in ["LABEL", "TGT"]):
    return "Label"
  if any(token in operand for token in ["MEM", "DS", "FLAT", "SMEM", "ATTR"]):
    return "Mem"
  if operand == "OPR_SRC":
    return "RegOrImm"
  return "Unknown"


def load_instructions(xml_path: str) -> Tuple[Dict[str, Tuple[str, str]], List[dict]]:
  root = ET.parse(xml_path).getroot()
  groups: Dict[str, Tuple[str, str]] = {}
  instructions: List[dict] = []
  for inst in root.iter("Instruction"):
    name = inst.findtext("InstructionName")
    if not name:
      continue
    fg = inst.find("FunctionalGroup")
    if fg is None:
      groups[name] = ("", "")
    else:
      group_name = fg.findtext("Name") or ""
      subgroup = fg.findtext("Subgroup") or ""
      groups[name] = (group_name, subgroup)

    encodings = [
      encoding.findtext("EncodingName") or ""
      for encoding in inst.findall("./InstructionEncodings/InstructionEncoding")
    ]
    encodings = sorted({name for name in encodings if name})

    encoding = inst.find("./InstructionEncodings/InstructionEncoding")
    operands = []
    if encoding is not None:
      ordered = []
      for operand in encoding.findall("./Operands/Operand"):
        if operand.attrib.get("IsImplicit", "").lower() == "true":
          continue
        order_text = operand.attrib.get("Order", "")
        try:
          order = int(order_text)
        except ValueError:
          order = 0
        ordered.append(
          (
            order,
            {
              "operand_type": operand.findtext("OperandType") or "",
              "data_format": operand.findtext("DataFormatName") or "",
              "size": operand.findtext("OperandSize") or "",
            },
          )
        )
      ordered.sort(key=lambda item: item[0])
      operands = [item[1] for item in ordered]
    instructions.append(
      {
        "name": name,
        "normalized_name": normalize_name(name),
        "operands": operands,
        "available_encodings": encodings,
      }
    )
  return groups, instructions


def generate(
  arch: str,
  out_dir: str,
  isa_xml: str | None,
  exclude_groups: List[str],
  exclude_vmem_subgroups: List[str],
) -> None:
  groups = None
  instructions: List[dict] = []
  if isa_xml and os.path.exists(isa_xml):
    groups, instructions = load_instructions(isa_xml)
  else:
    raise FileNotFoundError(isa_xml or "")

  excluded_groups = {name.upper() for name in exclude_groups}
  excluded_vmem = {name.upper() for name in exclude_vmem_subgroups}

  def is_allowed(inst: dict) -> bool:
    if groups is None:
      return True
    group_name, subgroup = groups.get(inst["name"], ("", ""))
    group_upper = group_name.upper()
    if group_upper in excluded_groups:
      return False
    if group_upper == "VMEM" and subgroup.upper() in excluded_vmem:
      return False
    return True

  instructions = [inst for inst in instructions if is_allowed(inst)]
  instructions.sort(key=lambda inst: inst["name"])
  names = [inst["normalized_name"] for inst in instructions]
  mapping = unique_variants(names)

  enum_lines = ["#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]", "pub enum Instruction {"]
  for inst in instructions:
    enum_lines.append(f"  {mapping[inst['normalized_name']]},")
  enum_lines.append("}")

  def_lines = ["pub struct InstructionDef {",
    "  pub name: &'static str,",
    "  pub instruction: Instruction,",
    "  pub args: &'static [ArgSpec],",
    "  pub encodings: &'static [&'static str],",
    "}",
  ]

  defs_lines = ["pub static INSTRUCTION_DEFS: &[InstructionDef] = &["]
  for inst in instructions:
    defs_lines.append(render_instruction_def(inst, mapping[inst["normalized_name"]]))
  defs_lines.append("];")

  lookup_pairs = sorted(mapping.items(), key=lambda item: item[0])
  lookup_lines = ["pub static INSTRUCTION_BY_NAME: &[(&str, Instruction)] = &["]
  for name, variant in lookup_pairs:
    lookup_lines.append(render_instruction_pair(name, variant))
  lookup_lines.append("];")

  lookup_fn = [
    "pub fn lookup(name: &str) -> Option<Instruction> {",
    "  INSTRUCTION_BY_NAME",
    "    .binary_search_by(|(n, _)| n.cmp(&name))",
    "    .ok()",
    "    .map(|idx| INSTRUCTION_BY_NAME[idx].1)",
    "}",
    "",
    "pub fn lookup_normalized(name: &str) -> Option<Instruction> {",
    "  lookup(&name.to_ascii_lowercase())",
    "}",
  ]

  out_path = os.path.join(out_dir, "generated.rs")
  os.makedirs(out_dir, exist_ok=True)
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("// Generated by scripts/gen_isa.py. Do not edit by hand.\n\n")
    f.write("use crate::isa::types::{ArgKind, ArgSpec, DataType};\n\n")
    f.write(f"pub const ARCH: &str = {rust_string_literal(arch)};\n\n")
    f.write("\n".join(enum_lines))
    f.write("\n\n")
    f.write("\n".join(def_lines))
    f.write("\n\n")
    f.write("\n".join(defs_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_fn))
    f.write("\n")


def main() -> None:
  parser = argparse.ArgumentParser(description="Generate RDNA ISA enums and lookup tables.")
  parser.add_argument("--isa-xml", default="./data/amdgpu_isa_rdna3_5.xml")
  parser.add_argument("--arch", default="rdna3.5")
  parser.add_argument("--out-dir", default="src/isa/rdna35")
  parser.add_argument("--exclude-groups", default="EXPORT")
  parser.add_argument("--exclude-vmem-subgroups", default="TEXTURE,SAMPLE,BVH")
  args = parser.parse_args()
  generate(
    args.arch,
    args.out_dir,
    args.isa_xml,
    parse_csv(args.exclude_groups),
    parse_csv(args.exclude_vmem_subgroups),
  )


if __name__ == "__main__":
  main()
