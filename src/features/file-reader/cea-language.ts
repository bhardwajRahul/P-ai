import type { LanguageRegistration } from "shiki";

function words(list: string[]) {
  return [...list].sort((left, right) => right.length - left.length).join("|");
}

const DIRECTIVES = words([
  "define", "alloc", "dealloc", "label", "assert",
  "registersymbol", "unregistersymbol", "include", "readmem",
  "fullaccess", "globalalloc", "createthreadandwait", "createthread",
  "loadbinary", "loadlibrary", "aobscanmodule", "aobscanregion", "aobscan",
  "reassemble", "unhook", "hook",
]);

const DATA = words(["db", "dw", "dd", "dq"]);

const SIZES = words(["byte", "word", "dword", "qword", "ptr", "short", "near", "far"]);

const INSTRUCTIONS = words([
  "movzx", "movsx", "pushad", "pushfd", "popad", "popfd",
  "repe", "repne", "retn", "imul", "idiv", "leave", "enter",
  "mov", "call", "push", "pop", "jmp", "ret", "cmp", "test", "lea",
  "xor", "add", "sub", "inc", "dec", "mul", "div", "shl", "shr", "sar",
  "rol", "ror", "nop", "cdq", "xchg", "neg", "sbb", "adc", "loop",
  "clc", "stc", "cld", "std", "int", "rep", "and", "or", "not",
  "jne", "je", "jae", "jbe", "jge", "jle", "jnz", "jns", "jcxz",
  "ja", "jb", "jg", "jl", "jz", "js", "jo", "jno",
  "setae", "setbe", "setne", "setge", "setle", "sete", "seta", "setb", "setg", "setl",
]);

const REGISTERS = words([
  "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "eip",
  "ax", "bx", "cx", "dx", "si", "di", "bp", "sp",
  "ah", "al", "bh", "bl", "ch", "cl", "dh", "dl",
]);

/** Cheat Engine Auto Assembler。只覆盖脚本阅读要看清的那一层，不覆盖 Lua / C 混合段。 */
export const ceaLanguage: LanguageRegistration = {
  name: "cea",
  scopeName: "source.cea",
  patterns: [
    { include: "#line-comment" },
    { include: "#block-comment" },
    { include: "#block-comment-c" },
    { include: "#string" },
    { include: "#section" },
    { include: "#directive" },
    { include: "#data" },
    { include: "#label" },
    { include: "#instruction" },
    { include: "#register" },
    { include: "#size" },
    { include: "#cast" },
    { include: "#decimal" },
    { include: "#number" },
  ],
  repository: {
    "line-comment": {
      match: "//.*",
      name: "comment.line.double-slash.cea",
    },
    "block-comment": {
      begin: "\\{",
      end: "\\}",
      name: "comment.block.cea",
    },
    "block-comment-c": {
      begin: "/\\*",
      end: "\\*/",
      name: "comment.block.cea",
    },
    string: {
      match: '"(?:\\\\.|[^"\\\\])*"',
      name: "string.quoted.double.cea",
    },
    section: {
      match: "(?i)\\[(?:enable|disable)\\]",
      name: "keyword.control.section.cea",
    },
    directive: {
      match: `\\b(?i:${DIRECTIVES})\\b`,
      name: "keyword.control.directive.cea",
    },
    data: {
      match: `\\b(?i:${DATA})\\b`,
      name: "storage.type.data.cea",
    },
    label: {
      match: `(?:^|[\\t ]+)([A-Za-z_]\\w*|[0-9A-Fa-f]{4,16})[\\t ]*(:)`,
      captures: {
        1: { name: "entity.name.label.cea" },
        2: { name: "punctuation.separator.label.cea" },
      },
    },
    instruction: {
      match: `\\b(?i:${INSTRUCTIONS})\\b`,
      name: "keyword.operator.instruction.cea",
    },
    register: {
      match: `\\b(?i:${REGISTERS})\\b`,
      name: "variable.language.register.cea",
    },
    size: {
      match: `\\b(?i:${SIZES})\\b`,
      name: "storage.modifier.size.cea",
    },
    cast: {
      match: "(?i)\\((?:int|float|double)\\)",
      name: "storage.type.cast.cea",
    },
    decimal: {
      match: "#\\d+\\b",
      name: "constant.numeric.decimal.cea",
    },
    number: {
      match: "\\b(?i:[0-9a-f]*[0-9][0-9a-f]*)\\b",
      name: "constant.numeric.hex.cea",
    },
  },
};
