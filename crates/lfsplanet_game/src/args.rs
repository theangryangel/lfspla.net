use std::path::PathBuf;

use clap::Args;

use crate::LayoutSpec;

#[derive(Args, Debug, Clone)]
pub struct InSimArgs {
    /// LFS InSim address (host:port).
    #[arg(long, default_value = "127.0.0.1:29999")]
    pub addr: String,

    /// InSim admin password, if the host requires one.
    #[arg(long)]
    pub admin_password: Option<String>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct LayoutArgs {
    /// Autocross layout name installed on the LFS server (loaded via /axload).
    #[arg(long, conflicts_with = "layout_file")]
    pub layout: Option<String>,

    /// Local LYT file to parse and load as AXM objects.
    #[arg(long, value_name = "PATH", conflicts_with = "layout")]
    pub layout_file: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
#[error("failed to read layout {}", path.display())]
pub struct LayoutArgsError {
    path: PathBuf,
    #[source]
    source: insim_lyt::Error,
}

impl TryFrom<LayoutArgs> for Option<LayoutSpec> {
    type Error = LayoutArgsError;

    fn try_from(args: LayoutArgs) -> Result<Self, Self::Error> {
        match (args.layout, args.layout_file) {
            (Some(name), None) => Ok(Some(LayoutSpec::Named(name))),
            (None, Some(path)) => {
                let layout =
                    insim_lyt::Lyt::from_path(&path).map_err(|source| LayoutArgsError {
                        path: path.clone(),
                        source,
                    })?;
                Ok(Some(LayoutSpec::Objects(layout.objects)))
            }
            (None, None) => Ok(None),
            (Some(_), Some(_)) => unreachable!("layout arguments conflict in clap"),
        }
    }
}
