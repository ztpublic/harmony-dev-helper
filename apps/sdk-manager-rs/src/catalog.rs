use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdkVersion {
    Api10,
    Api11,
    Api12,
    Api13,
    Api14,
    Api15,
    Api18,
    Api20,
}

impl SdkVersion {
    pub fn as_api_label(self) -> &'static str {
        match self {
            Self::Api10 => "API10",
            Self::Api11 => "API11",
            Self::Api12 => "API12",
            Self::Api13 => "API13",
            Self::Api14 => "API14",
            Self::Api15 => "API15",
            Self::Api18 => "API18",
            Self::Api20 => "API20",
        }
    }

    pub fn as_release_version(self) -> &'static str {
        match self {
            Self::Api10 => "4.0.0",
            Self::Api11 => "4.1.0",
            Self::Api12 => "5.0.0",
            Self::Api13 => "5.0.1",
            Self::Api14 => "5.0.2",
            Self::Api15 => "5.0.3",
            Self::Api18 => "5.1.0",
            Self::Api20 => "6.0.0",
        }
    }
}

impl fmt::Display for SdkVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_api_label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdkArch {
    X86,
    Arm,
}

impl SdkArch {
    pub fn current() -> Self {
        let arch = std::env::consts::ARCH;
        if arch.contains("arm") || arch.contains("aarch64") {
            Self::Arm
        } else {
            Self::X86
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::X86 => "X86",
            Self::Arm => "ARM",
        }
    }
}

impl fmt::Display for SdkArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdkOs {
    MacOs,
    Windows,
    Linux,
}

impl SdkOs {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "macos" => Self::MacOs,
            _ => Self::Linux,
        }
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::MacOs => "MacOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
        }
    }

    pub(crate) fn nested_archive_token(self) -> Option<&'static str> {
        match self {
            Self::Windows => Some("windows"),
            Self::Linux => Some("linux"),
            Self::MacOs => None,
        }
    }
}

impl fmt::Display for SdkOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_label())
    }
}

pub fn resolve_sdk_url(version: SdkVersion, arch: SdkArch, os: SdkOs) -> Option<&'static str> {
    match (version, arch, os) {
        (SdkVersion::Api10, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.0-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api10, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api10, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api10, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.0-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api10, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api10, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api11, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.1-Release/ohos-sdk-mac-public-signed.tar.gz")
        }
        (SdkVersion::Api11, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api11, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api11, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.1-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api11, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/4.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api11, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api12, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.0-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api12, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api12, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api12, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.0-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api12, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api12, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api13, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.1-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api13, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api13, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api13, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.1-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api13, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api13, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api14, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.2-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api14, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.2-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api14, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.2-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api14, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.2-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api14, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.2-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api14, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api15, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.3-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api15, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.3-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api15, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.3-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api15, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.3-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api15, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.0.3-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api15, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api18, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.1.0-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api18, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.1.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api18, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.1.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api18, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.1.0-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api18, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/5.1.0-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api18, SdkArch::Arm, SdkOs::Linux) => None,
        (SdkVersion::Api20, SdkArch::X86, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/ohos-sdk-mac-public.tar.gz")
        }
        (SdkVersion::Api20, SdkArch::X86, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api20, SdkArch::X86, SdkOs::Linux) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api20, SdkArch::Arm, SdkOs::MacOs) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/L2-SDK-MAC-M1-PUBLIC.tar.gz")
        }
        (SdkVersion::Api20, SdkArch::Arm, SdkOs::Windows) => {
            Some("https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/ohos-sdk-windows_linux-public.tar.gz")
        }
        (SdkVersion::Api20, SdkArch::Arm, SdkOs::Linux) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_sdk_url, SdkArch, SdkOs, SdkVersion};

    #[test]
    fn resolves_supported_urls() {
        assert_eq!(
            resolve_sdk_url(SdkVersion::Api20, SdkArch::X86, SdkOs::Linux),
            Some(
                "https://mirrors.huaweicloud.com/harmonyos/os/6.0.0.1-Release/ohos-sdk-windows_linux-public.tar.gz"
            )
        );
    }

    #[test]
    fn rejects_unsupported_combinations() {
        assert_eq!(
            resolve_sdk_url(SdkVersion::Api20, SdkArch::Arm, SdkOs::Linux),
            None
        );
    }
}
