from pathlib import Path


def replace(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected one refinement target, found {text.count(old)}")
    file.write_text(text.replace(old, new))


replace(
    "crates/tui/src/app.rs",
    '''    ) -> Self {\n        let selected = entries\n            .iter()\n            .position(|entry| entry.identity == current)\n            .unwrap_or(0);\n''',
    '''    ) -> Self {\n        let entries = if target == AssetPickerTarget::Trustline {\n            entries\n                .into_iter()\n                .filter(|entry| !entry.is_native())\n                .collect()\n        } else {\n            entries\n        };\n        let selected = entries\n            .iter()\n            .position(|entry| entry.identity == current)\n            .unwrap_or(0);\n''',
)

replace(
    "crates/tui/src/app.rs",
    '''    fn handle_asset_picker(&mut self, code: KeyCode) {\n        let mut chosen = None;\n        let mut cancelled = false;\n        if let Some(picker) = self.asset_picker.as_mut() {\n            match code {\n                KeyCode::Esc => cancelled = true,\n                KeyCode::Up | KeyCode::Char('k') => picker.previous(),\n                KeyCode::Down | KeyCode::Char('j') => picker.next(),\n                KeyCode::Enter => {\n                    chosen = picker\n                        .entries\n                        .get(picker.selected)\n                        .map(|entry| (picker.target, entry.identity.clone()));\n                }\n                _ => {}\n            }\n        }\n\n        if cancelled {\n            self.asset_picker = None;\n            self.status = "Asset selection cancelled; manual value kept".to_owned();\n        } else if let Some((target, identity)) = chosen {\n            self.asset_picker = None;\n            self.apply_asset_identity(target, &identity);\n            self.status = format!("Selected exact asset identity {identity}");\n        }\n    }\n''',
    '''    fn handle_asset_picker(&mut self, code: KeyCode) {\n        let mut chosen = None;\n        let mut cancelled = false;\n        let mut refresh_request = None;\n        if let Some(picker) = self.asset_picker.as_mut() {\n            match code {\n                KeyCode::Esc => cancelled = true,\n                KeyCode::Up | KeyCode::Char('k') => picker.previous(),\n                KeyCode::Down | KeyCode::Char('j') => picker.next(),\n                KeyCode::Char('r') => {\n                    let current = picker\n                        .entries\n                        .get(picker.selected)\n                        .map(|entry| entry.identity.clone())\n                        .unwrap_or_default();\n                    refresh_request = Some((picker.target, current));\n                }\n                KeyCode::Enter => {\n                    chosen = picker\n                        .entries\n                        .get(picker.selected)\n                        .map(|entry| (picker.target, entry.identity.clone()));\n                }\n                _ => {}\n            }\n        }\n\n        if let Some((target, current)) = refresh_request {\n            match self.client.asset_catalog(MAX_ASSET_CATALOG_LIMIT, true) {\n                Ok(snapshot) => {\n                    let refreshed = snapshot.refreshed;\n                    self.asset_picker =\n                        Some(AssetPickerState::new(snapshot.entries, target, &current));\n                    self.status = if refreshed {\n                        "Asset catalog refreshed".to_owned()\n                    } else {\n                        "Refresh unavailable; cached/manual assets remain".to_owned()\n                    };\n                }\n                Err(error) => self.status = error,\n            }\n            return;\n        }\n\n        if cancelled {\n            self.asset_picker = None;\n            self.status = "Asset selection cancelled; manual value kept".to_owned();\n        } else if let Some((target, identity)) = chosen {\n            self.asset_picker = None;\n            self.apply_asset_identity(target, &identity);\n            self.status = format!("Selected exact asset identity {identity}");\n        }\n    }\n''',
)

replace(
    "crates/tui/src/app.rs",
    '''            match self.client.asset_catalog(MAX_ASSET_CATALOG_LIMIT, true) {\n                Ok(snapshot) => {\n                    self.asset_picker = Some(AssetPickerState::new(snapshot.entries, target, &current));\n                    self.status = "Choose an exact asset identity; Esc keeps manual entry".to_owned();\n                }\n''',
    '''            match self.client.asset_catalog(MAX_ASSET_CATALOG_LIMIT, false) {\n                Ok(snapshot) => {\n                    self.asset_picker = Some(AssetPickerState::new(snapshot.entries, target, &current));\n                    self.status =\n                        "Cached asset catalog · r refresh · Esc keeps manual entry".to_owned();\n                }\n''',
)

replace(
    "crates/tui/src/render.rs",
    '"Up/Down or j/k select   Enter choose exact identity   Esc keep manual value"',
    '"Up/Down or j/k select   r refresh   Enter choose exact identity   Esc keep manual value"',
)

replace(
    "crates/tui/src/main.rs",
    '''    #[test]\n    fn asset_picker_applies_full_issuer_identity() {''',
    '''    #[test]\n    fn trustline_picker_excludes_native_asset() {\n        let picker = AssetPickerState::new(\n            vec![\n                catalog_entry("XLM"),\n                catalog_entry("USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"),\n            ],\n            AssetPickerTarget::Trustline,\n            "XLM",\n        );\n        assert_eq!(picker.entries.len(), 1);\n        assert!(!picker.entries[0].is_native());\n    }\n\n    #[test]\n    fn asset_picker_applies_full_issuer_identity() {''',
)
