//! The app's own assets (icon, illustrations) and the Lucide icons it uses, over the component library's set.
//!
//! Only the icons listed in `icon_assets!` are embedded; add a name there before using it.

use std::borrow::Cow;

use gpui_kit::assets::icon_assets;
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    pub UsedIcons,
    [
        Sparkles,
        Keyboard,
        WandSparkles,
        PanelsTopLeft,
        BookOpenText,
        ChartColumn,
        Command,
        SlidersHorizontal,
        Info,
        ChevronRight,
        ChevronLeft,
        ChevronDown,
        ChevronUp,
        ChevronsUpDown,
        Check,
        X,
        Plus,
        Minus,
        Search,
        CircleAlert,
        RotateCcw,
        ExternalLink,
        ArrowDown,
        ArrowUp,
        ArrowLeft,
        ArrowRight,
    ]
);

pub struct Assets;

const OWN: &[(&str, &[u8])] = &[("app-icon.png", include_bytes!("../assets/app-icon.png"))];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = OWN.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        if let Some(bytes) = UsedIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut list = gpui_kit::assets::Assets.list(path)?;
        list.extend(UsedIcons.list(path)?);
        list.extend(OWN.iter().filter(|(name, _)| name.starts_with(path)).map(|(name, _)| SharedString::from(*name)));
        list.sort();
        list.dedup();
        Ok(list)
    }
}
