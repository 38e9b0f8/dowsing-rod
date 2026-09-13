//! Extension dispatch shared by discovery, parsing, and candidate partitioning.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Python,
    #[serde(rename = "typescript")]
    TypeScript,
    #[serde(rename = "javascript")]
    JavaScript,
    C,
    Cpp,
    #[serde(rename = "csharp")]
    CSharp,
    Verilog,
    SystemVerilog,
    Vhdl,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        // Uppercase .C conventionally denotes C++, unlike lowercase .c.
        if extension == "C" {
            return Some(Self::Cpp);
        }
        Some(match extension.to_ascii_lowercase().as_str() {
            "py" | "pyi" => Self::Python,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "c" | "h" => Self::C,
            "cc" | "cpp" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++" | "ipp" | "tpp" => {
                Self::Cpp
            }
            "cs" => Self::CSharp,
            "v" | "vh" => Self::Verilog,
            "sv" | "svh" => Self::SystemVerilog,
            "vhd" | "vhdl" => Self::Vhdl,
            _ => return None,
        })
    }

    pub fn is_hdl(self) -> bool {
        matches!(self, Self::Verilog | Self::SystemVerilog | Self::Vhdl)
    }

    pub(crate) fn grammar(self, path: &Path) -> tree_sitter::Language {
        match self {
            Self::TypeScript
                if path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("tsx") || e.eq_ignore_ascii_case("jsx")
                }) =>
            {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            }
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Verilog | Self::SystemVerilog => tree_sitter_verilog::LANGUAGE.into(),
            Self::Vhdl => tree_sitter_vhdl::LANGUAGE.into(),
            Self::Python => unreachable!("Python uses RustPython"),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Verilog => "verilog",
            Self::SystemVerilog => "system_verilog",
            Self::Vhdl => "vhdl",
        })
    }
}
