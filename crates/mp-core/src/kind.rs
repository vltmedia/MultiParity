/// File kind, derived from the extension. Used for display, filters and claiming rules.
pub fn kind_of(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "fbx" | "obj" | "gltf" | "glb" | "blend" | "ma" | "mb" | "max" | "c4d" | "usd" | "usda"
        | "usdc" | "usdz" | "ztl" | "spp" | "3ds" | "dae" | "stl" | "ply" => "model",
        "png" | "tga" | "tif" | "tiff" | "dds" | "ktx" | "ktx2" | "sbsar" | "sbs" => "texture",
        "jpg" | "jpeg" | "psd" | "psb" | "exr" | "hdr" | "webp" | "gif" | "bmp" | "kra" | "svg"
        | "ai" | "afphoto" | "afdesign" => "image",
        "wav" | "mp3" | "ogg" | "flac" | "aif" | "aiff" | "m4a" | "bank" | "wem" => "audio",
        "mov" | "mp4" | "mkv" | "avi" | "webm" | "mxf" | "prores" => "video",
        "umap" | "unity" | "tscn" | "scn" | "level" => "level",
        "uasset" | "prefab" | "asset" | "mat" | "tres" | "res" | "hip" | "hiplc" | "nk"
        | "aep" | "prproj" | "drp" | "fla" => "scene",
        "abc" | "vdb" | "bgeo" | "sc" | "bphys" => "cache",
        "rs" | "c" | "cc" | "cpp" | "h" | "hpp" | "cs" | "js" | "ts" | "tsx" | "jsx" | "py"
        | "lua" | "gd" | "go" | "java" | "kt" | "swift" | "m" | "mm" | "shader" | "hlsl"
        | "glsl" | "usf" | "ush" | "cginc" | "wgsl" => "code",
        "txt" | "md" | "json" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "xml" | "csv"
        | "uproject" | "uplugin" | "meta" | "html" | "css" => "text",
        _ => "other",
    }
}

/// Text kinds merge line by line; everything else is binary and is claimed instead.
pub fn is_text_kind(kind: &str) -> bool {
    matches!(kind, "code" | "text")
}

/// Whether a file of this path should be claimed automatically on first edit.
pub fn is_claimable(path: &str) -> bool {
    !is_text_kind(kind_of(path))
}

/// Heuristic used before a text merge: no NUL bytes in the first 8 KiB and valid UTF-8.
pub fn looks_like_text(data: &[u8]) -> bool {
    !data.iter().take(8192).any(|b| *b == 0) && std::str::from_utf8(data).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds() {
        assert_eq!(kind_of("Characters/Hero/hero_body.FBX"), "model");
        assert_eq!(kind_of("a/b.uasset"), "scene");
        assert_eq!(kind_of("noext"), "other");
        assert!(is_claimable("x.psd"));
        assert!(!is_claimable("Source/Game.cpp"));
    }
}
