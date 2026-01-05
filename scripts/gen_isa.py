#!/usr/bin/env python
import argparse
import os
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from typing import Dict, List, Tuple

MAX_LINE = 150
COMPACT_LINE = 300

TYPE_SUFFIXES = ["f16", "f32", "bf16", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"]


@dataclass(frozen=True)
class ArchConfig:
  arch: str
  isa_xml: str
  out_dir: str


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


def safe_fn_base(name: str) -> str:
  out = []
  for ch in name:
    if ch.isalnum():
      out.append(ch.lower())
    else:
      out.append("_")
  return "".join(out)


def unique_fn_names(names: List[str]) -> Dict[str, str]:
  seen: Dict[str, int] = {}
  mapping: Dict[str, str] = {}
  for name in names:
    base = safe_fn_base(name)
    count = seen.get(base, 0)
    seen[base] = count + 1
    if count == 0:
      mapping[name] = base
    else:
      mapping[name] = f"{base}_{count + 1}"
  return mapping


def render_common_def(inst: dict) -> str:
  name = rust_string_literal(inst["normalized_name"])
  args = format_arg_specs(inst.get("operands", []), inst["normalized_name"])
  dual_args = "&[]"
  if inst.get("is_v_dual"):
    dual_args = format_arg_specs(inst.get("operands", []), inst["normalized_name"])
  supports_modifiers = "true" if inst.get("supports_modifiers", False) else "false"
  return (
    "InstructionCommonDef { "
    f"name: {name}, "
    f"args: {args}, "
    f"dual_args: {dual_args}, "
    f"supports_modifiers: {supports_modifiers} "
    "}"
  )


def render_instruction_ref(variant: str, common_ref: str) -> str:
  return f"InstructionDef {{ instruction: Instruction::{variant}, common: {common_ref} }}"


def format_arg_specs(operands: List[dict], inst_name: str = "") -> str:
  if not operands:
    return "&[]"
  rendered = [render_arg_spec(operand, inst_name) for operand in operands]
  return f"&[{', '.join(rendered)}]"


def render_arg_spec(operand: dict, inst_name: str = "") -> str:
  operand_type = operand.get("operand_type", "")
  kind = operand_kind(operand_type)

  # Handle OPR_SRC context-dependently based on instruction prefix
  if operand_type.upper() == "OPR_SRC":
    if inst_name.lower().startswith("v_"):
      kind = "VgprOrImm"
    elif inst_name.lower().startswith("s_"):
      kind = "SgprOrImm"
    # else keep as determined by operand_kind

  if kind in {"Imm", "SgprOrImm", "VgprOrImm"}:
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


def instruction_key(inst: dict) -> str:
  return inst["normalized_name"]


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


def pack_items(items: List[str], indent: int, max_len: int) -> List[str]:
  if not items:
    return []
  lines = []
  prefix = " " * indent
  current = ""
  for item in items:
    entry = f"{item},"
    if not current:
      current = prefix + entry
      continue
    candidate = current + " " + entry
    if len(candidate) <= max_len:
      current = candidate
    else:
      lines.append(current)
      current = prefix + entry
  if current:
    lines.append(current)
  return lines


def unique_instruction_count(instructions: List[dict]) -> int:
  names = [inst["normalized_name"] for inst in instructions]
  unique = set()
  for name in names:
    base, dtype = extract_base_and_type(name)
    unique.add(base if dtype else name)
  return len(unique)


def parse_csv(value: str) -> List[str]:
  return [item.strip() for item in value.split(",") if item.strip()]


def normalize_name(name: str) -> str:
  return name.lower()


def extract_base_and_type(name: str) -> tuple:
  """
  Extract operation base and data type from instruction name.

  Examples:
      v_add_f16 → ("v_add", "f16")
      v_add_f32 → ("v_add", "f32")
      v_and_b32 → ("v_and_b32", None)  # bitwise keeps suffix
      s_mov_b32 → ("s_mov_b32", None)  # not a type variant
  """
  for suffix in TYPE_SUFFIXES:
    if name.endswith(f"_{suffix}"):
      base = name[:-len(suffix)-1]
      # Bitwise ops keep suffix (not genericizable)
      if any(op in base for op in ["_and_", "_or_", "_xor_", "_not_"]):
        return (name, None)
      return (base, suffix)
  return (name, None)


def instruction_category(name: str) -> str:
  if name.startswith("v_"):
    return "v"
  if name.startswith("s_"):
    return "s"
  if name.startswith(("ds_", "buffer_", "flat_", "global_", "image_")):
    return "mem"
  return "misc"


def build_typed_bases(instructions: List[dict]) -> Dict[str, set]:
  bases: Dict[str, Dict[str, set]] = {"v": {}, "s": {}, "mem": {}, "misc": {}}
  for inst in instructions:
    name = inst["normalized_name"]
    base, dtype = extract_base_and_type(name)
    if not dtype:
      continue
    category = instruction_category(name)
    bases.setdefault(category, {})
    bases[category].setdefault(base, set()).add(dtype)
  typed = {}
  for category, base_map in bases.items():
    typed[category] = {base for base, dtypes in base_map.items() if len(dtypes) > 1}
  return typed


def operand_kind(operand_type: str) -> str:
  operand = operand_type.upper()

  # Vector register or immediate (for v_* instructions)
  if "VGPR_OR_INLINE" in operand or operand in {"OPR_VSRC", "OPR_VSRC0", "OPR_VSRC1", "OPR_VSRC2"}:
    return "VgprOrImm"

  # Vector registers only
  if "VGPR" in operand or "VDST" in operand:
    return "Vgpr"

  # Special registers
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

  # Scalar sources and memory offsets accept scalar registers or immediates
  if operand in {"OPR_SSRC", "OPR_SSRC0", "OPR_SSRC1", "OPR_SSRC2", "OPR_SMEM_OFFSET", "OPR_OFFSET"}:
    return "SgprOrImm"

  # Scalar registers only (destinations, etc.)
  if any(token in operand for token in ["SGPR", "SREG", "SDST", "SCC", "M0"]):
    return "Sgpr"

  # Pure immediates
  if any(token in operand for token in ["SIMM", "LIT", "INLINE", "IMM", "SENDMSG", "WAITCNT", "VERSION", "CLAUSE"]):
    return "Imm"

  # Labels
  if any(token in operand for token in ["LABEL", "TGT"]):
    return "Label"

  # Memory operands
  if any(token in operand for token in ["MEM", "DS", "FLAT", "SMEM", "ATTR"]):
    return "Mem"

  # Generic source - shouldn't happen, but default to SgprOrImm
  if operand == "OPR_SRC":
    return "SgprOrImm"

  return "Unknown"


def supports_modifiers(enc_name: str) -> bool:
  """Determine if an encoding supports abs/neg modifiers."""
  enc = enc_name.upper()

  # VOP3 and VOP3P support both abs and neg
  if 'VOP3P' in enc:
    return True
  if 'VOP3' in enc:
    # VOP3 supports modifiers, but exclude VOP3P (already handled above)
    return True

  # SDWA supports both (CDNA only, but we'll allow it)
  if 'SDWA' in enc:
    return True

  # All other encodings don't support modifiers
  return False


def load_instructions(
  xml_path: str,
  exclude_groups: List[str],
  exclude_vmem_subgroups: List[str],
) -> List[dict]:
  if not os.path.exists(xml_path):
    raise FileNotFoundError(xml_path)
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

    # Prefer standard encodings over DPP/SDWA variants
    # Prefer VOP3 over VOP2/VOP1 to support modifiers (abs/neg)
    # Standard encodings typically have more permissive operand types (OPR_SRC vs OPR_VGPR)
    all_encodings = inst.findall("./InstructionEncodings/InstructionEncoding")
    preferred_encodings = ["ENC_VOP3", "ENC_VOP1", "ENC_VOP2", "ENC_SOP1", "ENC_SOP2", "ENC_SOPC", "ENC_SOPK", "ENC_SOPP", "ENC_SMEM", "ENC_VMEM"]

    # Select encoding based on preference order (not XML order)
    encoding = None
    for pref in preferred_encodings:
      for enc in all_encodings:
        enc_name = enc.findtext("EncodingName") or ""
        if pref in enc_name:
          encoding = enc
          break
      if encoding is not None:
        break

    # Fallback to first encoding if no preferred encoding found
    if encoding is None and all_encodings:
      encoding = all_encodings[0]

    operands = []
    enc_name = ""
    if encoding is not None:
      enc_name = encoding.findtext("EncodingName") or ""
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

    # Determine modifier support from encoding
    supports_mods_by_encoding = supports_modifiers(enc_name)

    def is_float_data_format(value: str) -> bool:
      upper = value.upper()
      return any(token in upper for token in ["F16", "F32", "F64", "BF16"])

    def name_has_float_hint(value: str) -> bool:
      lower = value.lower()
      return any(token in lower for token in ["_f16", "_f32", "_f64", "_bf16", "fma"])

    is_float_instruction = name_has_float_hint(name) or any(
      is_float_data_format(op.get("data_format") or "") for op in operands
    )
    supports_mods = supports_mods_by_encoding and is_float_instruction

    instructions.append(
      {
        "name": name,
        "normalized_name": normalize_name(name),
        "operands": operands,
        "is_v_dual": normalize_name(name).startswith("v_dual_"),
        "supports_modifiers": supports_mods,
      }
    )

  excluded_groups = {name.upper() for name in exclude_groups}
  excluded_vmem = {name.upper() for name in exclude_vmem_subgroups}

  def is_f64_instruction(inst: dict) -> bool:
    if "F64" in inst["name"].upper():
      return True
    for operand in inst.get("operands", []):
      data_format = (operand.get("data_format") or "").upper()
      if "F64" in data_format:
        return True
    return False

  def is_allowed(inst: dict) -> bool:
    group_name, subgroup = groups.get(inst["name"], ("", ""))
    group_upper = group_name.upper()
    if group_upper in excluded_groups:
      return False
    if group_upper == "VMEM" and subgroup.upper() in excluded_vmem:
      return False
    if is_f64_instruction(inst):
      return False
    return True

  return [inst for inst in instructions if is_allowed(inst)]


def build_common_names(arch_instructions: Dict[str, List[dict]]) -> set:
  common = None
  for insts in arch_instructions.values():
    names = {instruction_key(inst) for inst in insts}
    common = names if common is None else common & names
  return common or set()


def generate_base(
  common_insts: List[dict],
  out_dir: str,
) -> Dict[str, int]:
  common_insts = sorted(common_insts, key=lambda inst: inst["normalized_name"])
  defs_lines = ["pub static INSTRUCTION_COMMON_DEFS: &[InstructionCommonDef] = &["]
  def_items = [render_common_def(inst) for inst in common_insts]
  defs_lines.extend(pack_items(def_items, 2, COMPACT_LINE))
  defs_lines.append("];")

  lookup_pairs = [(inst["normalized_name"], idx) for idx, inst in enumerate(common_insts)]
  lookup_pairs.sort(key=lambda item: item[0])
  lookup_lines = ["pub static INSTRUCTION_COMMON_BY_NAME: &[(&str, usize)] = &["]
  lookup_items = [f"({rust_string_literal(name)}, {idx})" for name, idx in lookup_pairs]
  lookup_lines.extend(pack_items(lookup_items, 2, COMPACT_LINE))
  lookup_lines.append("];")

  lookup_fn = [
    "pub fn lookup_common(name: &str) -> Option<&'static InstructionCommonDef> {",
    "  INSTRUCTION_COMMON_BY_NAME",
    "    .binary_search_by(|(n, _)| n.cmp(&name))",
    "    .ok()",
    "    .map(|idx| &INSTRUCTION_COMMON_DEFS[INSTRUCTION_COMMON_BY_NAME[idx].1])",
    "}",
    "",
    "pub fn lookup_common_normalized(name: &str) -> Option<&'static InstructionCommonDef> {",
    "  lookup_common(&name.to_ascii_lowercase())",
    "}",
  ]

  out_path = os.path.join(out_dir, "generated.rs")
  os.makedirs(out_dir, exist_ok=True)
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("// Generated by scripts/gen_isa.py. Do not edit by hand.\n\n")
    f.write("use crate::isa::types::{ArgKind, ArgSpec, DataType, InstructionCommonDef};\n\n")
    f.write("\n".join(defs_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_fn))
    f.write("\n")

  return {instruction_key(inst): idx for idx, inst in enumerate(common_insts)}


def generate_arch(
  config: ArchConfig,
  instructions: List[dict],
  common_names: set,
  base_index_map: Dict[str, int],
) -> None:
  instructions = sorted(instructions, key=lambda inst: inst["normalized_name"])
  names = [inst["normalized_name"] for inst in instructions]
  mapping = unique_variants(names)

  arch_common = [inst for inst in instructions if instruction_key(inst) not in common_names]
  arch_common = sorted(arch_common, key=lambda inst: inst["normalized_name"])
  arch_index_map = {instruction_key(inst): idx for idx, inst in enumerate(arch_common)}

  enum_lines = ["#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]", "pub enum Instruction {"]
  enum_items = [mapping[inst["normalized_name"]] for inst in instructions]
  enum_lines.extend(pack_items(enum_items, 2, COMPACT_LINE))
  enum_lines.append("}")

  arch_defs = ["pub static ARCH_COMMON_DEFS: &[InstructionCommonDef] = &["]
  arch_items = [render_common_def(inst) for inst in arch_common]
  arch_defs.extend(pack_items(arch_items, 2, COMPACT_LINE))
  arch_defs.append("];")

  def_lines = ["pub static INSTRUCTION_DEFS: &[InstructionDef<Instruction>] = &["]
  def_items = []
  for inst in instructions:
    name = instruction_key(inst)
    if name in base_index_map:
      common_ref = f"&base::INSTRUCTION_COMMON_DEFS[{base_index_map[name]}]"
    else:
      common_ref = f"&ARCH_COMMON_DEFS[{arch_index_map[name]}]"
    def_items.append(render_instruction_ref(mapping[inst["normalized_name"]], common_ref))
  def_lines.extend(pack_items(def_items, 2, COMPACT_LINE))
  def_lines.append("];")

  lookup_pairs = sorted(mapping.items(), key=lambda item: item[0])
  lookup_lines = ["pub static INSTRUCTION_BY_NAME: &[(&str, Instruction)] = &["]
  lookup_items = [f"({rust_string_literal(name)}, Instruction::{variant})" for name, variant in lookup_pairs]
  lookup_lines.extend(pack_items(lookup_items, 2, COMPACT_LINE))
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
    "",
    "pub fn lookup_common_def(name: &str) -> Option<&'static InstructionCommonDef> {",
    "  let instruction = lookup_normalized(name)?;",
    "  INSTRUCTION_DEFS",
    "    .iter()",
    "    .find(|def| def.instruction == instruction)",
    "    .map(|def| def.common)",
    "}",
  ]

  out_path = os.path.join(config.out_dir, "generated.rs")
  os.makedirs(config.out_dir, exist_ok=True)
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("// Generated by scripts/gen_isa.py. Do not edit by hand.\n\n")
    f.write("use crate::isa::base;\n")
    f.write("use crate::isa::types::{ArgKind, ArgSpec, DataType, InstructionCommonDef, InstructionDef};\n\n")
    f.write(f"pub const ARCH: &str = {rust_string_literal(config.arch)};\n\n")
    f.write("\n".join(enum_lines))
    f.write("\n\n")
    f.write("\n".join(arch_defs))
    f.write("\n\n")
    f.write("\n".join(def_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_lines))
    f.write("\n\n")
    f.write("\n".join(lookup_fn))
    f.write("\n")


def ops_module_name(config: ArchConfig) -> str:
  return os.path.basename(config.out_dir)


def is_memory_instruction(name: str) -> bool:
  """Check if instruction is a memory operation"""
  # Check for memory keywords
  if any(kw in name for kw in ["load", "store", "atomic"]):
    return True
  # Check for memory prefixes
  prefixes = ["buffer_", "ds_", "flat_", "global_", "image_", "tbuffer_"]
  if any(name.startswith(p) for p in prefixes):
    return True
  # Scalar memory loads
  if name.startswith("s_load") or name.startswith("s_buffer_load"):
    return True
  return False


def is_basic_control_flow(name: str) -> bool:
  """Check if instruction is basic control flow (branches, calls, endpgm, etc.)"""
  patterns = [
    "s_endpgm", "s_sendmsg", "s_barrier", "s_waitcnt",
    "s_branch", "s_cbranch", "s_setpc", "s_swappc",
    "s_call", "s_rfe", "s_getpc", "s_setprio",
    "s_sleep", "s_trap", "s_sethalt", "s_setkill",
    "s_wait_", "s_wakeup",
  ]
  return any(name.startswith(p) for p in patterns)


def is_exec_mask_instruction(name: str) -> bool:
  """Check if instruction is execution mask management"""
  patterns = [
    "s_and_not0_saveexec", "s_and_not1_saveexec", "s_and_saveexec",
    "s_and_not0_wrexec", "s_and_not1_wrexec",
    "s_or_not0_saveexec", "s_or_not1_saveexec", "s_or_saveexec",
    "s_nand_saveexec", "s_nor_saveexec",
    "s_xnor_saveexec", "s_xor_saveexec",
  ]
  return any(name.startswith(p) for p in patterns)


def is_quad_mode_instruction(name: str) -> bool:
  """Check if instruction is quad/wave mode control"""
  return name.startswith("s_wqm_") or name.startswith("s_quadmask_")


def is_system_state_instruction(name: str) -> bool:
  """Check if instruction is system state management"""
  patterns = [
    "s_dcache_inv", "s_icache_inv",
    "s_denorm_mode", "s_round_mode",
    "s_delay_alu",
    "s_incperflevel", "s_decperflevel",
  ]
  return any(name.startswith(p) for p in patterns)


def is_special_register_instruction(name: str) -> bool:
  """Check if instruction is special register access"""
  return name.startswith("s_getreg_") or name.startswith("s_setreg_")


def is_relative_addressing_instruction(name: str) -> bool:
  """Check if instruction is relative addressing"""
  return name.startswith("s_movreld_") or name.startswith("s_movrels")


def is_control_flow_instruction(name: str) -> bool:
  """Check if instruction is control flow or system control"""
  patterns = [
    # Original control flow
    "s_endpgm", "s_sendmsg", "s_barrier", "s_waitcnt",
    "s_branch", "s_cbranch", "s_setpc", "s_swappc",
    "s_call", "s_rfe", "s_getpc", "s_setprio",
    "s_sleep", "s_trap", "s_sethalt", "s_setkill",
    "s_wait_", "s_wakeup",

    # Execution mask management
    "s_and_not0_saveexec", "s_and_not1_saveexec", "s_and_saveexec",
    "s_and_not0_wrexec", "s_and_not1_wrexec",
    "s_or_not0_saveexec", "s_or_not1_saveexec", "s_or_saveexec",
    "s_nand_saveexec", "s_nor_saveexec",
    "s_xnor_saveexec", "s_xor_saveexec",

    # Quad/wave mode
    "s_wqm_", "s_quadmask_",

    # System state
    "s_dcache_inv", "s_icache_inv",
    "s_denorm_mode", "s_round_mode",
    "s_delay_alu",
    "s_incperflevel", "s_decperflevel",

    # Special registers
    "s_getreg_", "s_setreg_",

    # Relative addressing
    "s_movreld_", "s_movrels",

    # Tracing/debugging/utility
    "s_ttracedata", "s_code_end", "s_version",
    "s_nop", "s_clause",
  ]
  return any(name.startswith(p) for p in patterns)


def categorize_instructions(instructions: List[dict]) -> Dict[str, List[dict]]:
  """Categorize instructions into vector, scalar, memory, and sys_control"""
  categories = {"vector": [], "scalar": [], "memory": [], "sys_control": []}

  for inst in instructions:
    name = inst["normalized_name"]

    # Memory: detect by keywords and prefixes
    if is_memory_instruction(name):
      categories["memory"].append(inst)
    # System/Control: BEFORE scalar check
    elif is_control_flow_instruction(name):
      categories["sys_control"].append(inst)
    # Vector ALU
    elif name.startswith("v_"):
      categories["vector"].append(inst)
    # Scalar ALU (only what's left)
    elif name.startswith("s_"):
      categories["scalar"].append(inst)
    # Everything else
    else:
      categories["sys_control"].append(inst)

  return categories


def write_manual_ops_file(out_path: str, instruction_names: List[str]) -> None:
  if not instruction_names:
    if os.path.exists(out_path):
      os.remove(out_path)
    return
  fn_map = unique_fn_names(instruction_names)
  lines = ["use crate::sim::{Ctx, ExecError, ExecResult};", ""]
  for name in instruction_names:
    fn_name = fn_map[name]
    lines.append(f"pub fn {fn_name}(ctx: &mut Ctx) -> ExecResult {{")
    lines.append("  let _ = ctx;")
    lines.append(f"  Err(ExecError::Unimplemented({rust_string_literal(name)}))")
    lines.append("}")
    lines.append("")
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))


def generate_ops_module(
  module_name: str,
  instructions: List[dict],
  out_dir: str,
  typed_bases: Dict[str, set],
) -> None:
  os.makedirs(out_dir, exist_ok=True)

  # Map operation bases to BinaryOp enum variants (vector only)
  V_BINARY_OPS = {
    "v_add": "Add",
    "v_sub": "Sub",
    "v_mul": "Mul",
    "v_div": "Div",
    "v_min": "Min",
    "v_max": "Max",
  }

  V_CMP_OPS = {
    "v_cmp_eq": ("Eq", False),
    "v_cmp_ne": ("Ne", False),
    "v_cmp_lt": ("Lt", False),
    "v_cmp_le": ("Le", False),
    "v_cmp_gt": ("Gt", False),
    "v_cmp_ge": ("Ge", False),
    "v_cmp_lg": ("Lg", False),
    "v_cmp_neq": ("Neq", False),
    "v_cmp_nlt": ("Nlt", False),
    "v_cmp_nle": ("Nle", False),
    "v_cmp_ngt": ("Ngt", False),
    "v_cmp_nge": ("Nge", False),
    "v_cmp_nlg": ("Nlg", False),
    "v_cmp_o": ("O", False),
    "v_cmp_u": ("U", False),
    "v_cmpx_eq": ("Eq", True),
    "v_cmpx_ne": ("Ne", True),
    "v_cmpx_lt": ("Lt", True),
    "v_cmpx_le": ("Le", True),
    "v_cmpx_gt": ("Gt", True),
    "v_cmpx_ge": ("Ge", True),
    "v_cmpx_lg": ("Lg", True),
    "v_cmpx_neq": ("Neq", True),
    "v_cmpx_nlt": ("Nlt", True),
    "v_cmpx_nle": ("Nle", True),
    "v_cmpx_ngt": ("Ngt", True),
    "v_cmpx_nge": ("Nge", True),
    "v_cmpx_nlg": ("Nlg", True),
    "v_cmpx_o": ("O", True),
    "v_cmpx_u": ("U", True),
  }

  DTYPE_MAP = {
    "f16": "F16",
    "f32": "F32",
    "bf16": "BF16",
    "i8": "I8",
    "i16": "I16",
    "i32": "I32",
    "i64": "I64",
    "u8": "U8",
    "u16": "U16",
    "u32": "U32",
    "u64": "U64",
  }

  typed_ops = []
  manual_by_category = {"v": [], "s": [], "mem": [], "misc": []}

  for inst in sorted(instructions, key=lambda inst: inst["normalized_name"]):
    name = inst["normalized_name"]
    base, dtype = extract_base_and_type(name)
    category = instruction_category(name)
    is_typed = dtype is not None and base in typed_bases.get(category, set())
    if is_typed and dtype in DTYPE_MAP:
      dt = DTYPE_MAP[dtype]
      if category == "v":
        if base in V_BINARY_OPS:
          typed_ops.append((name, "V_BINARY", dt, V_BINARY_OPS[base], False))
        elif base in V_CMP_OPS:
          op, update_exec = V_CMP_OPS[base]
          typed_ops.append((name, "V_CMP", dt, op, update_exec))
        else:
          typed_ops.append((name, "V_UNKNOWN", dt, "", False))
      elif category == "s":
        typed_ops.append((name, "S_UNKNOWN", dt, "", False))
      elif category == "mem":
        typed_ops.append((name, "MEM_UNKNOWN", dt, "", False))
      else:
        manual_by_category["misc"].append(name)
    else:
      manual_by_category[category].append(name)

  manual_files = {
    "v": "manual_v_ops.rs",
    "s": "manual_s_ops.rs",
    "mem": "manual_mem_ops.rs",
    "misc": "manual_misc_ops.rs",
  }

  manual_modules = []
  manual_ops = []
  for category, names in manual_by_category.items():
    out_path = os.path.join(out_dir, manual_files[category])
    write_manual_ops_file(out_path, names)
    if names:
      module_name = manual_files[category][:-3]
      manual_modules.append(module_name)
      fn_map = unique_fn_names(names)
      for name in names:
        manual_ops.append((name, fn_map[name], module_name))

  manual_ops.sort(key=lambda x: x[0])
  typed_ops.sort(key=lambda x: x[0])

  uses_v_binary = any(kind == "V_BINARY" for _, kind, _, _, _ in typed_ops)
  uses_v_cmp = any(kind == "V_CMP" for _, kind, _, _, _ in typed_ops)
  uses_typed = len(typed_ops) > 0

  lines = ["use crate::sim::Handler;"]
  if uses_typed:
    lines.append("use crate::ops::typed::TypedHandler;")
    lines.append("use crate::isa::types::DataType;")
    lines.append("use crate::ops::typed_mem_ops::TypedMemHandler;")
    lines.append("use crate::ops::typed_s_ops::TypedSHandler;")
    lines.append("use crate::ops::typed_v_ops::TypedVHandler;")
  if uses_v_binary:
    lines.append("use crate::ops::typed_v_ops::BinaryOp;")
  if uses_v_cmp:
    lines.append("use crate::ops::typed_v_ops::CmpOp;")
  lines.append("")

  for module in manual_modules:
    lines.append(f"pub mod {module};")
  if manual_modules:
    lines.append("")

  lines.append("pub static OPS: &[(&str, Handler)] = &[")
  for inst_name, fn_name, module_name in manual_ops:
    lines.append(f"  ({rust_string_literal(inst_name)}, {module_name}::{fn_name}),")
  lines.append("];")
  lines.append("")

  lines.append("pub static TYPED_OPS: &[(&str, TypedHandler)] = &[")
  for inst_name, kind, dt, op, update_exec in typed_ops:
    if kind == "V_BINARY":
      lines.append(f"  ({rust_string_literal(inst_name)}, TypedHandler::V(TypedVHandler::Binary {{ dtype: DataType::{dt}, op: BinaryOp::{op} }})),")
    elif kind == "V_CMP":
      update_exec_str = "true" if update_exec else "false"
      lines.append(
        f"  ({rust_string_literal(inst_name)}, TypedHandler::V(TypedVHandler::Cmp {{ dtype: DataType::{dt}, op: CmpOp::{op}, update_exec: {update_exec_str} }})),"
      )
    elif kind == "S_UNKNOWN":
      lines.append(f"  ({rust_string_literal(inst_name)}, TypedHandler::S(TypedSHandler::Unknown {{ dtype: DataType::{dt} }})),")
    elif kind == "MEM_UNKNOWN":
      lines.append(f"  ({rust_string_literal(inst_name)}, TypedHandler::Mem(TypedMemHandler::Unknown {{ dtype: DataType::{dt} }})),")
    else:
      lines.append(f"  ({rust_string_literal(inst_name)}, TypedHandler::V(TypedVHandler::Unknown {{ dtype: DataType::{dt} }})),")
  lines.append("];")

  mod_path = os.path.join(out_dir, "mod.rs")
  with open(mod_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))
    f.write("\n")


def generate_ops_mod(out_dir: str, module_names: List[str]) -> None:
  lines = [
    "pub mod typed;",
    "pub mod typed_mem_ops;",
    "pub mod typed_s_ops;",
    "pub mod typed_v_ops;",
  ]
  for name in module_names:
    lines.append(f"pub mod {name};")
  lines.append("")
  for name in module_names:
    alias = name.upper()
    lines.append(f"pub use {name}::OPS as {alias}_OPS;")
    lines.append(f"pub use {name}::TYPED_OPS as {alias}_TYPED_OPS;")
  lines.append("")

  out_path = os.path.join(out_dir, "mod.rs")
  os.makedirs(out_dir, exist_ok=True)
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))
    f.write("\n")


def generate_types(out_dir: str) -> None:
  lines = [
    "// Generated by scripts/gen_isa.py. Do not edit by hand.",
    "",
    "#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]",
    "pub enum ArgKind {",
    "  Sgpr,",
    "  Vgpr,",
    "  SgprOrImm,  // Scalar register or immediate (for s_* instructions)",
    "  VgprOrImm,  // Vector register or immediate (for v_* instructions)",
    "  Imm,",
    "  Mem,",
    "  Label,",
    "  Special,",
    "  Unknown,",
    "}",
    "",
    "#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]",
    "pub enum DataType {",
    "  None,",
    "  Any,",
    "  Unknown,",
    "  B1,",
    "  B8,",
    "  B16,",
    "  B32,",
    "  B64,",
    "  B96,",
    "  B128,",
    "  F16,",
    "  F32,",
    "  F64,",
    "  BF16,",
    "  I8,",
    "  I16,",
    "  I24,",
    "  I32,",
    "  I64,",
    "  U8,",
    "  U16,",
    "  U24,",
    "  U32,",
    "  U64,",
    "  M64,",
    "  Pk2B16,",
    "  Pk2BF16,",
    "  Pk2F16,",
    "  Pk2I16,",
    "  Pk2U16,",
    "  Pk2U8,",
    "  Pk4B8,",
    "  Pk4IU8,",
    "  Pk4U8,",
    "  Pk8IU4,",
    "  Pk8U4,",
    "}",
    "",
    "#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]",
    "pub struct ArgSpec {",
    "  pub kind: ArgKind,",
    "  pub data_type: DataType,",
    "  pub width: u16,",
    "}",
    "",
    "#[derive(Copy, Clone, Debug)]",
    "pub struct InstructionCommonDef {",
    "  pub name: &'static str,",
    "  pub args: &'static [ArgSpec],",
    "  pub dual_args: &'static [ArgSpec],",
    "  pub supports_modifiers: bool,",
    "}",
    "",
    "#[derive(Copy, Clone, Debug)]",
    "pub struct InstructionDef<I> {",
    "  pub instruction: I,",
    "  pub common: &'static InstructionCommonDef,",
    "}",
    "",
  ]

  out_path = os.path.join(out_dir, "types.rs")
  os.makedirs(out_dir, exist_ok=True)
  with open(out_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))
    f.write("\n")


def main() -> None:
  parser = argparse.ArgumentParser(description="Generate RDNA ISA enums and lookup tables.")
  parser.add_argument("--arch", action="append", nargs=3, metavar=("NAME", "XML", "OUT_DIR"))
  parser.add_argument("--base-out-dir", default="src/isa/base")
  parser.add_argument("--ops-out-dir", default="src/ops")
  parser.add_argument("--exclude-groups", default="EXPORT")
  parser.add_argument("--exclude-vmem-subgroups", default="TEXTURE,SAMPLE,BVH")
  parser.add_argument("--write-ops", action="store_true", help="overwrite src/ops generated stubs")
  args = parser.parse_args()
  if args.arch:
    arch_configs = [ArchConfig(arch, xml, out_dir) for arch, xml, out_dir in args.arch]
  else:
    arch_configs = [
      ArchConfig("rdna3", "./data/amdgpu_isa_rdna3.xml", "src/isa/rdna3"),
      ArchConfig("rdna3.5", "./data/amdgpu_isa_rdna3_5.xml", "src/isa/rdna35"),
      ArchConfig("rdna4", "./data/amdgpu_isa_rdna4.xml", "src/isa/rdna4"),
    ]
  exclude_groups = parse_csv(args.exclude_groups)
  exclude_vmem = parse_csv(args.exclude_vmem_subgroups)
  arch_instructions = {
    config.arch: load_instructions(config.isa_xml, exclude_groups, exclude_vmem)
    for config in arch_configs
  }
  common_names = build_common_names(arch_instructions)

  # Generate types.rs first since other files depend on it
  types_dir = os.path.dirname(args.base_out_dir)  # src/isa/base -> src/isa
  generate_types(types_dir)

  reference_arch = arch_configs[0].arch
  common_insts = [
    inst for inst in arch_instructions[reference_arch]
    if instruction_key(inst) in common_names
  ]
  base_index_map = generate_base(common_insts, args.base_out_dir)
  for config in arch_configs:
    generate_arch(
      config,
      arch_instructions[config.arch],
      common_names,
      base_index_map,
    )

  arch_specific = {
    config.arch: [
      inst for inst in arch_instructions[config.arch]
      if instruction_key(inst) not in common_names
    ]
    for config in arch_configs
  }
  typed_bases = build_typed_bases([inst for insts in arch_instructions.values() for inst in insts])
  if args.write_ops:
    warning = (
      "\x1b[31mWARNING: --write-ops will overwrite all op definitions in src/ops. "
      "Type 'yes' to continue:\x1b[0m "
    )
    if input(warning).strip().lower() != "yes":
      print("Aborting --write-ops.")
      return
    generate_ops_module("base", common_insts, os.path.join(args.ops_out_dir, "base"), typed_bases)
    module_names = ["base"]
    for config in arch_configs:
      module_name = ops_module_name(config)
      module_names.append(module_name)
      generate_ops_module(
        module_name,
        arch_specific[config.arch],
        os.path.join(args.ops_out_dir, module_name),
        typed_bases,
      )
    generate_ops_mod(args.ops_out_dir, module_names)

  print(f"base: {unique_instruction_count(common_insts)} instructions")
  for config in arch_configs:
    total = unique_instruction_count(arch_instructions[config.arch])
    unique = unique_instruction_count(arch_specific[config.arch])
    print(f"{config.arch}: {total} instructions ({unique} arch-specific)")


if __name__ == "__main__":
  main()
