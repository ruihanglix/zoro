// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

#[cfg(target_os = "macos")]
use std::collections::HashMap;

#[cfg(target_os = "macos")]
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter, Wry,
};

#[cfg(not(target_os = "macos"))]
use tauri::AppHandle;

/// Return a map of menu translation keys → translated strings for the given language.
/// Falls back to English for unknown language codes.
#[cfg(target_os = "macos")]
fn get_translations(lang: &str) -> HashMap<&'static str, &'static str> {
    match lang {
        "zh-CN" => HashMap::from([
            ("file", "文件"),
            ("add_paper", "添加论文"),
            ("open_library", "打开文库"),
            ("import", "导入…"),
            ("settings", "设置"),
            ("quit", "退出 Zoro"),
            ("edit", "编辑"),
            ("view", "显示"),
            ("zoom_in", "放大"),
            ("zoom_out", "缩小"),
            ("actual_size", "实际大小"),
            ("toggle_sidebar", "切换侧边栏"),
            ("library", "文库"),
            ("feed", "订阅"),
            ("papers_cool", "Papers Cool"),
            ("theme", "主题"),
            ("theme_light", "浅色"),
            ("theme_dark", "深色"),
            ("theme_system", "跟随系统"),
            ("window", "窗口"),
            ("minimize", "最小化"),
            ("zoom", "缩放"),
            ("full_screen", "全屏"),
            ("bring_all_to_front", "全部置于最前"),
            ("help", "帮助"),
            ("about", "关于 Zoro"),
            ("check_updates", "检查更新…"),
            ("github", "GitHub"),
            ("website", "官方网站"),
            ("report_issue", "报告问题"),
        ]),
        "ja" => HashMap::from([
            ("file", "ファイル"),
            ("add_paper", "論文を追加"),
            ("open_library", "ライブラリを開く"),
            ("import", "インポート…"),
            ("settings", "設定"),
            ("quit", "Zoro を終了"),
            ("edit", "編集"),
            ("view", "表示"),
            ("zoom_in", "拡大"),
            ("zoom_out", "縮小"),
            ("actual_size", "実際のサイズ"),
            ("toggle_sidebar", "サイドバーの切替"),
            ("library", "ライブラリ"),
            ("feed", "フィード"),
            ("papers_cool", "Papers Cool"),
            ("theme", "テーマ"),
            ("theme_light", "ライト"),
            ("theme_dark", "ダーク"),
            ("theme_system", "システム"),
            ("window", "ウインドウ"),
            ("minimize", "最小化"),
            ("zoom", "ズーム"),
            ("full_screen", "フルスクリーン"),
            ("bring_all_to_front", "すべてを手前に移動"),
            ("help", "ヘルプ"),
            ("about", "Zoro について"),
            ("check_updates", "アップデートを確認…"),
            ("github", "GitHub"),
            ("website", "ウェブサイト"),
            ("report_issue", "問題を報告"),
        ]),
        "ko" => HashMap::from([
            ("file", "파일"),
            ("add_paper", "논문 추가"),
            ("open_library", "라이브러리 열기"),
            ("import", "가져오기…"),
            ("settings", "설정"),
            ("quit", "Zoro 종료"),
            ("edit", "편집"),
            ("view", "보기"),
            ("zoom_in", "확대"),
            ("zoom_out", "축소"),
            ("actual_size", "실제 크기"),
            ("toggle_sidebar", "사이드바 토글"),
            ("library", "라이브러리"),
            ("feed", "피드"),
            ("papers_cool", "Papers Cool"),
            ("theme", "테마"),
            ("theme_light", "라이트"),
            ("theme_dark", "다크"),
            ("theme_system", "시스템"),
            ("window", "윈도우"),
            ("minimize", "최소화"),
            ("zoom", "확대/축소"),
            ("full_screen", "전체 화면"),
            ("bring_all_to_front", "모두 앞으로 가져오기"),
            ("help", "도움말"),
            ("about", "Zoro에 관하여"),
            ("check_updates", "업데이트 확인…"),
            ("github", "GitHub"),
            ("website", "웹사이트"),
            ("report_issue", "문제 신고"),
        ]),
        "es" => HashMap::from([
            ("file", "Archivo"),
            ("add_paper", "Añadir artículo"),
            ("open_library", "Abrir biblioteca"),
            ("import", "Importar…"),
            ("settings", "Ajustes"),
            ("quit", "Salir de Zoro"),
            ("edit", "Edición"),
            ("view", "Visualización"),
            ("zoom_in", "Ampliar"),
            ("zoom_out", "Reducir"),
            ("actual_size", "Tamaño real"),
            ("toggle_sidebar", "Alternar barra lateral"),
            ("library", "Biblioteca"),
            ("feed", "Feed"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Tema"),
            ("theme_light", "Claro"),
            ("theme_dark", "Oscuro"),
            ("theme_system", "Sistema"),
            ("window", "Ventana"),
            ("minimize", "Minimizar"),
            ("zoom", "Zoom"),
            ("full_screen", "Pantalla completa"),
            ("bring_all_to_front", "Traer todo al frente"),
            ("help", "Ayuda"),
            ("about", "Acerca de Zoro"),
            ("check_updates", "Buscar actualizaciones…"),
            ("github", "GitHub"),
            ("website", "Sitio web"),
            ("report_issue", "Informar de un problema"),
        ]),
        "fr" => HashMap::from([
            ("file", "Fichier"),
            ("add_paper", "Ajouter un article"),
            ("open_library", "Ouvrir la bibliothèque"),
            ("import", "Importer…"),
            ("settings", "Préférences"),
            ("quit", "Quitter Zoro"),
            ("edit", "Édition"),
            ("view", "Présentation"),
            ("zoom_in", "Agrandir"),
            ("zoom_out", "Réduire"),
            ("actual_size", "Taille réelle"),
            ("toggle_sidebar", "Afficher/masquer la barre latérale"),
            ("library", "Bibliothèque"),
            ("feed", "Flux"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Thème"),
            ("theme_light", "Clair"),
            ("theme_dark", "Sombre"),
            ("theme_system", "Système"),
            ("window", "Fenêtre"),
            ("minimize", "Réduire"),
            ("zoom", "Zoom"),
            ("full_screen", "Plein écran"),
            ("bring_all_to_front", "Tout ramener au premier plan"),
            ("help", "Aide"),
            ("about", "À propos de Zoro"),
            ("check_updates", "Rechercher les mises à jour…"),
            ("github", "GitHub"),
            ("website", "Site web"),
            ("report_issue", "Signaler un problème"),
        ]),
        "de" => HashMap::from([
            ("file", "Ablage"),
            ("add_paper", "Artikel hinzufügen"),
            ("open_library", "Bibliothek öffnen"),
            ("import", "Importieren …"),
            ("settings", "Einstellungen"),
            ("quit", "Zoro beenden"),
            ("edit", "Bearbeiten"),
            ("view", "Darstellung"),
            ("zoom_in", "Vergrößern"),
            ("zoom_out", "Verkleinern"),
            ("actual_size", "Originalgröße"),
            ("toggle_sidebar", "Seitenleiste ein-/ausblenden"),
            ("library", "Bibliothek"),
            ("feed", "Feed"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Erscheinungsbild"),
            ("theme_light", "Hell"),
            ("theme_dark", "Dunkel"),
            ("theme_system", "System"),
            ("window", "Fenster"),
            ("minimize", "Minimieren"),
            ("zoom", "Zoomen"),
            ("full_screen", "Vollbild"),
            ("bring_all_to_front", "Alle nach vorne bringen"),
            ("help", "Hilfe"),
            ("about", "Über Zoro"),
            ("check_updates", "Nach Updates suchen …"),
            ("github", "GitHub"),
            ("website", "Webseite"),
            ("report_issue", "Problem melden"),
        ]),
        "pt" => HashMap::from([
            ("file", "Ficheiro"),
            ("add_paper", "Adicionar artigo"),
            ("open_library", "Abrir biblioteca"),
            ("import", "Importar…"),
            ("settings", "Definições"),
            ("quit", "Sair do Zoro"),
            ("edit", "Edição"),
            ("view", "Visualização"),
            ("zoom_in", "Ampliar"),
            ("zoom_out", "Reduzir"),
            ("actual_size", "Tamanho real"),
            ("toggle_sidebar", "Alternar barra lateral"),
            ("library", "Biblioteca"),
            ("feed", "Feed"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Tema"),
            ("theme_light", "Claro"),
            ("theme_dark", "Escuro"),
            ("theme_system", "Sistema"),
            ("window", "Janela"),
            ("minimize", "Minimizar"),
            ("zoom", "Zoom"),
            ("full_screen", "Ecrã inteiro"),
            ("bring_all_to_front", "Trazer tudo para a frente"),
            ("help", "Ajuda"),
            ("about", "Sobre o Zoro"),
            ("check_updates", "Procurar atualizações…"),
            ("github", "GitHub"),
            ("website", "Website"),
            ("report_issue", "Reportar problema"),
        ]),
        "ru" => HashMap::from([
            ("file", "Файл"),
            ("add_paper", "Добавить статью"),
            ("open_library", "Открыть библиотеку"),
            ("import", "Импорт…"),
            ("settings", "Настройки"),
            ("quit", "Завершить Zoro"),
            ("edit", "Правка"),
            ("view", "Вид"),
            ("zoom_in", "Увеличить"),
            ("zoom_out", "Уменьшить"),
            ("actual_size", "Фактический размер"),
            ("toggle_sidebar", "Боковая панель"),
            ("library", "Библиотека"),
            ("feed", "Лента"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Тема"),
            ("theme_light", "Светлая"),
            ("theme_dark", "Тёмная"),
            ("theme_system", "Системная"),
            ("window", "Окно"),
            ("minimize", "Свернуть"),
            ("zoom", "Изменить масштаб"),
            ("full_screen", "Полный экран"),
            ("bring_all_to_front", "Все окна — на передний план"),
            ("help", "Справка"),
            ("about", "О программе Zoro"),
            ("check_updates", "Проверить обновления…"),
            ("github", "GitHub"),
            ("website", "Веб-сайт"),
            ("report_issue", "Сообщить о проблеме"),
        ]),
        // English (default / fallback)
        _ => HashMap::from([
            ("file", "File"),
            ("add_paper", "Add Paper"),
            ("open_library", "Open Library"),
            ("import", "Import…"),
            ("settings", "Settings"),
            ("quit", "Quit Zoro"),
            ("edit", "Edit"),
            ("view", "View"),
            ("zoom_in", "Zoom In"),
            ("zoom_out", "Zoom Out"),
            ("actual_size", "Actual Size"),
            ("toggle_sidebar", "Toggle Sidebar"),
            ("library", "Library"),
            ("feed", "Feed"),
            ("papers_cool", "Papers Cool"),
            ("theme", "Theme"),
            ("theme_light", "Light"),
            ("theme_dark", "Dark"),
            ("theme_system", "System"),
            ("window", "Window"),
            ("minimize", "Minimize"),
            ("zoom", "Zoom"),
            ("full_screen", "Full Screen"),
            ("bring_all_to_front", "Bring All to Front"),
            ("help", "Help"),
            ("about", "About Zoro"),
            ("check_updates", "Check for Updates…"),
            ("github", "GitHub"),
            ("website", "Website"),
            ("report_issue", "Report an Issue"),
        ]),
    }
}

/// Look up a translation key; returns the key itself if not found.
#[cfg(target_os = "macos")]
fn t<'a>(translations: &'a HashMap<&'static str, &'static str>, key: &'a str) -> &'a str {
    translations.get(key).copied().unwrap_or(key)
}

/// Build the full native menu bar for macOS.
#[cfg(target_os = "macos")]
pub fn build_menu(app: &AppHandle, lang: &str) -> Result<tauri::menu::Menu<Wry>, tauri::Error> {
    let tr = get_translations(lang);

    // --- App menu (macOS convention: first submenu = app name) ---
    let app_menu = SubmenuBuilder::new(app, "Zoro")
        .about(None)
        .separator()
        .item(
            &MenuItemBuilder::with_id("settings", t(&tr, "settings"))
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    // --- File menu ---
    let file_menu = SubmenuBuilder::new(app, t(&tr, "file"))
        .item(&MenuItemBuilder::with_id("add-paper", t(&tr, "add_paper")).build(app)?)
        .item(&MenuItemBuilder::with_id("open-library", t(&tr, "open_library")).build(app)?)
        .item(&MenuItemBuilder::with_id("import", t(&tr, "import")).build(app)?)
        .separator()
        .close_window()
        .build()?;

    // --- Edit menu (all predefined so macOS auto-localises) ---
    let edit_menu = SubmenuBuilder::new(app, t(&tr, "edit"))
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .build()?;

    // --- View menu ---
    let theme_submenu = SubmenuBuilder::new(app, t(&tr, "theme"))
        .item(&MenuItemBuilder::with_id("theme-light", t(&tr, "theme_light")).build(app)?)
        .item(&MenuItemBuilder::with_id("theme-dark", t(&tr, "theme_dark")).build(app)?)
        .item(&MenuItemBuilder::with_id("theme-system", t(&tr, "theme_system")).build(app)?)
        .build()?;

    let view_menu = SubmenuBuilder::new(app, t(&tr, "view"))
        .item(
            &MenuItemBuilder::with_id("zoom-in", t(&tr, "zoom_in"))
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom-out", t(&tr, "zoom_out"))
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("actual-size", t(&tr, "actual_size")).build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle-sidebar", t(&tr, "toggle_sidebar"))
                .accelerator("CmdOrCtrl+\\")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("view-library", t(&tr, "library")).build(app)?)
        .item(&MenuItemBuilder::with_id("view-feed", t(&tr, "feed")).build(app)?)
        .item(&MenuItemBuilder::with_id("view-papers-cool", t(&tr, "papers_cool")).build(app)?)
        .separator()
        .item(&theme_submenu)
        .build()?;

    // --- Window menu ---
    let window_menu = SubmenuBuilder::new(app, t(&tr, "window"))
        .minimize()
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;

    // --- Help menu ---
    let help_menu = SubmenuBuilder::new(app, t(&tr, "help"))
        .item(&MenuItemBuilder::with_id("about", t(&tr, "about")).build(app)?)
        .item(&MenuItemBuilder::with_id("check-updates", t(&tr, "check_updates")).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("github", t(&tr, "github")).build(app)?)
        .item(&MenuItemBuilder::with_id("website", t(&tr, "website")).build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("report-issue", t(&tr, "report_issue")).build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}

/// Register a global menu-event handler that forwards custom menu item clicks
/// to the frontend via a `"menu-event"` event.
#[cfg(target_os = "macos")]
pub fn register_menu_event_handler(app: &AppHandle) {
    let handle = app.clone();
    app.on_menu_event(move |_app, event| {
        let id = event.id().0.as_str();
        // Only forward our custom IDs; predefined items (undo/copy/etc.)
        // are handled natively by macOS.
        match id {
            "add-paper" | "open-library" | "import" | "settings" | "zoom-in" | "zoom-out"
            | "actual-size" | "toggle-sidebar" | "view-library" | "view-feed"
            | "view-papers-cool" | "theme-light" | "theme-dark" | "theme-system" | "about"
            | "check-updates" | "github" | "website" | "report-issue" => {
                let _ = handle.emit("menu-event", id);
            }
            _ => {}
        }
    });
}

/// Tauri command: rebuild the native menu with the given language.
/// On non-macOS platforms this is a no-op.
#[tauri::command]
pub fn set_menu_language(app: AppHandle, lang: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let menu = build_menu(&app, &lang).map_err(|e| e.to_string())?;
        app.set_menu(menu).map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&app, &lang);
    }
    Ok(())
}
