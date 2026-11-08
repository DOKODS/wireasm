//! Localization: every visible string lives here, in English and Russian.
//! The language is picked from the system locale unless the user overrides it
//! in Settings.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
        }
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().split(['-', '_']).next()? {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            _ => None,
        }
    }

    /// System locale, falling back to English.
    pub fn detect() -> Lang {
        sys_locale::get_locale()
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or(Lang::En)
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
        }
    }
}

/// key, English, Russian
pub const STRINGS: &[(&str, &str, &str)] = &[
    ("app.title", "WireStudio — WireASM IDE", "WireStudio — среда WireASM"),
    ("menu.file", "File", "Файл"),
    ("menu.edit", "Edit", "Правка"),
    ("menu.view", "View", "Вид"),
    ("menu.nodes", "Nodes", "Узлы"),
    ("menu.help", "Help", "Справка"),
    ("menu.language", "Language", "Язык"),
    ("file.new", "New graph", "Новый граф"),
    ("file.open", "Open…", "Открыть…"),
    ("file.save", "Save", "Сохранить"),
    ("file.save_as", "Save as…", "Сохранить как…"),
    ("file.export_asm", "Export assembler…", "Экспорт ассемблера…"),
    ("file.import_library", "Import node library…", "Импорт библиотеки узлов…"),
    ("file.export_library", "Export node library…", "Экспорт библиотеки узлов…"),
    ("file.import_asm", "Import .s as node…", "Импорт .s как узла…"),
    ("file.examples", "Examples", "Примеры"),
    ("file.quit", "Quit", "Выход"),
    ("edit.undo", "Undo", "Отменить"),
    ("edit.redo", "Redo", "Вернуть"),
    ("edit.delete", "Delete node", "Удалить узел"),
    ("edit.fit", "Fit graph", "Вписать граф"),
    ("edit.zoom_in", "Zoom in", "Приблизить"),
    ("edit.zoom_out", "Zoom out", "Отдалить"),
    ("edit.compile", "Compile", "Скомпилировать"),
    ("edit.build_run", "Build & run", "Собрать и запустить"),
    ("edit.variables", "Variables…", "Переменные…"),
    ("edit.custom_node", "New custom node…", "Новый свой узел…"),
    ("edit.shortcuts", "Keyboard shortcuts", "Горячие клавиши"),
    ("help.about", "About WireStudio", "О WireStudio"),
    ("toolbar.target", "Target", "Цель"),
    ("toolbar.compile", "Compile", "Скомпилировать"),
    ("toolbar.run", "Build & run", "Собрать и запустить"),
    ("toolbar.fit", "Fit", "Вписать"),
    ("toolbar.untitled", "untitled", "без имени"),
    ("toolbar.dirty", "unsaved changes", "есть изменения"),
    ("palette.title", "Nodes", "Узлы"),
    ("palette.search", "Search nodes…", "Поиск узлов…"),
    ("palette.flow", "Flow", "Поток"),
    ("palette.values", "Values", "Значения"),
    ("palette.output", "Output", "Вывод"),
    ("palette.system", "System", "Система"),
    ("palette.notes", "Notes", "Заметки"),
    ("palette.custom", "Custom", "Свои"),
    ("palette.empty", "Nothing matches the search.", "Ничего не найдено."),
    ("palette.hint", "Click a node to add it, or drag it onto the canvas.", "Нажмите узел, чтобы добавить, или перетащите его на холст."),
    ("inspector.title", "Inspector", "Инспектор"),
    ("inspector.empty", "Select a node to edit it.", "Выберите узел, чтобы изменить его."),
    ("inspector.node", "Node", "Узел"),
    ("inspector.id", "ID", "ID"),
    ("inspector.kind", "Kind", "Тип"),
    ("inspector.position", "Position", "Позиция"),
    ("inspector.properties", "Properties", "Свойства"),
    ("inspector.var_name", "Variable", "Переменная"),
    ("inspector.value", "Value", "Значение"),
    ("inspector.operator", "Operator", "Операция"),
    ("inspector.register", "Register", "Регистр"),
    ("inspector.function", "Function", "Функция"),
    ("inspector.syscall_number", "Syscall number", "Номер сисколла"),
    ("inspector.text", "Text", "Текст"),
    ("inspector.edit_definition", "Edit custom definition…", "Изменить описание узла…"),
    ("inspector.delete", "Delete node", "Удалить узел"),
    ("inspector.no_props", "This node has no editable properties.", "У этого узла нет редактируемых свойств."),
    ("inspector.custom_node", "Custom node", "Свой узел"),
    ("inspector.prop_key", "Property key", "Ключ свойства"),
    ("inspector.add", "Add", "Добавить"),
    ("vars.title", "Variables", "Переменные"),
    ("vars.name", "Name", "Имя"),
    ("vars.init", "Initial value", "Начальное значение"),
    ("vars.add", "Add variable", "Добавить переменную"),
    ("vars.remove", "Remove", "Удалить"),
    ("vars.empty", "No variables yet.", "Переменных пока нет."),
    ("vars.close", "Close", "Закрыть"),
    ("vars.hint", "Variables live in the graph and become stack slots in the generated assembler.", "Переменные хранятся в графе и становятся слотами стека в готовом ассемблере."),
    ("custom.new", "New custom node", "Новый свой узел"),
    ("custom.edit", "Edit custom node", "Свой узел"),
    ("custom.kind", "Kind", "Тип"),
    ("custom.title", "Title", "Название"),
    ("custom.category", "Category", "Категория"),
    ("custom.description", "Description", "Описание"),
    ("custom.inputs", "Inputs", "Входы"),
    ("custom.outputs", "Outputs", "Выходы"),
    ("custom.add_input", "Add input", "Добавить вход"),
    ("custom.add_output", "Add output", "Добавить выход"),
    ("custom.type_u64", "u64", "u64"),
    ("custom.type_exec", "exec", "exec"),
    ("custom.pin_name", "Pin name", "Имя пина"),
    ("custom.aarch64", "AArch64 template", "Шаблон AArch64"),
    ("custom.x86_64", "x86-64 template", "Шаблон x86-64"),
    ("custom.contract", "Contract: inputs {in0}..{in7} (alias {a},{b}), result {out}, unique prefix {label}, property {prop:key}. AArch64 inputs x0..x7; x86-64 inputs rdi,rsi,rdx,rcx,r8,r9,r10,r11.", "Контракт: входы {in0}..{in7} (алиасы {a},{b}), результат {out}, уникальный префикс {label}, свойство {prop:key}. AArch64 — входы x0..x7; x86-64 — rdi,rsi,rdx,rcx,r8,r9,r10,r11."),
    ("custom.save", "Save node", "Сохранить узел"),
    ("custom.cancel", "Cancel", "Отмена"),
    ("custom.delete", "Delete definition", "Удалить описание"),
    ("custom.error_kind", "Kind must start with a letter or '_' and contain only letters, digits, '_' or '-'.", "Тип должен начинаться с буквы или '_' и содержать только буквы, цифры, '_' или '-'."),
    ("custom.error_builtin", "This kind is a built-in node.", "Этот тип занят встроенным узлом."),
    ("custom.error_asm", "Both assembler templates must be non-empty.", "Оба шаблона ассемблера должны быть заполнены."),
    ("custom.error_used", "This node is used in the graph. Delete or replace its instances first.", "Узел используется в графе. Сначала удалите или замените его экземпляры."),
    ("custom.error_pin", "Pins need unique, non-empty names; up to 8 u64 inputs, one u64 output, one exec pin per side, and an exec output needs an exec input.", "Пины должны иметь уникальные непустые имена; до 8 входов u64, один выход u64, один exec-пин с каждой стороны; exec-выход требует exec-входа."),
    ("custom.error_exists", "Another custom node already uses this kind.", "Узел с таким типом уже существует."),
    ("bottom.asm", "Assembler", "Ассемблер"),
    ("bottom.problems", "Problems", "Проблемы"),
    ("bottom.console", "Console", "Консоль"),
    ("bottom.save_asm", "Save .s…", "Сохранить .s…"),
    ("bottom.copy", "Copy", "Копировать"),
    ("bottom.clear", "Clear", "Очистить"),
    ("bottom.no_problems", "No problems. The graph is valid.", "Проблем нет. Граф корректен."),
    ("bottom.no_asm", "Compile the graph to see assembler here.", "Скомпилируйте граф, чтобы увидеть ассемблер."),
    ("bottom.no_output", "Build & run something to see its output.", "Запустите сборку, чтобы увидеть вывод."),
    ("bottom.stats", "{} nodes · {} edges · {} lines", "{} узлов · {} связей · {} строк"),
    ("severity.error", "error", "ошибка"),
    ("severity.warning", "warning", "предупреждение"),
    ("severity.info", "info", "инфо"),
    ("status.ready", "Ready.", "Готово."),
    ("status.console_ready", "WireStudio ready.", "WireStudio готова."),
    ("status.unknown_target", "Unknown target. Check the graph's target field.", "Неизвестная цель. Проверьте поле target в графе."),
    ("status.new", "New graph. Add an Entry node first.", "Новый граф. Сначала добавьте узел Entry."),
    ("status.one_entry", "Only one Entry node is allowed.", "Допускается только один узел Entry."),
    ("status.opened", "Opened {}", "Открыт {}"),
    ("status.saved", "Saved {}", "Сохранён {}"),
    ("status.saved_asm", "Assembler written to {}", "Ассемблер записан в {}"),
    ("status.compiled", "Compiled for {}", "Скомпилировано для {}"),
    ("status.compile_failed", "Compilation failed: {} error(s).", "Компиляция не удалась: ошибок — {}."),
    ("status.running", "Building and running…", "Сборка и запуск…"),
    ("status.run_done", "Program finished with status {}.", "Программа завершилась со статусом {}."),
    ("status.run_failed", "Build or run failed.", "Сборка или запуск не удались."),
    ("status.host_only", "Target {} cannot run on this host. Use Export assembler.", "Цель {} не запускается на этой машине. Используйте экспорт ассемблера."),
    ("status.imported_lib", "Imported {} node definition(s) from {}.", "Импортировано описаний узлов: {} из {}."),
    ("status.exported_lib", "Library written to {}.", "Библиотека записана в {}."),
    ("status.imported_asm", "Imported {} as a custom node. Check its pins and templates.", "{} импортирован как свой узел. Проверьте пины и шаблоны."),
    ("status.no_custom", "This graph has no custom nodes to export.", "В графе нет своих узлов для экспорта."),
    ("status.cancelled", "Cancelled.", "Отменено."),
    ("status.deleted", "Deleted {}.", "Удалён {}."),
    ("status.copied", "Copied to the clipboard.", "Скопировано в буфер обмена."),
    ("status.no_clipboard", "Clipboard is unavailable.", "Буфер обмена недоступен."),
    ("status.target_changed", "Target set to {}.", "Цель: {}."),
    ("status.auto_saved", "Assembler updated: {}", "Ассемблер обновлён: {}"),
    ("status.custom_saved", "Custom node '{}' saved.", "Свой узел «{}» сохранён."),
    ("dialog.graph", "WireASM graph", "Граф WireASM"),
    ("dialog.library", "WireASM library", "Библиотека WireASM"),
    ("dialog.asm", "Assembler", "Ассемблер"),
    ("dialog.asm_source", "Assembler source", "Исходник ассемблера"),
    ("unsaved.title", "Unsaved changes", "Несохранённые изменения"),
    ("unsaved.text", "The graph has unsaved changes. Save before continuing?", "В графе есть несохранённые изменения. Сохранить перед продолжением?"),
    ("unsaved.save", "Save", "Сохранить"),
    ("unsaved.discard", "Discard", "Не сохранять"),
    ("unsaved.cancel", "Cancel", "Отмена"),
    ("about.title", "About WireStudio", "О WireStudio"),
    ("about.text", "WireASM {} — a visual assembler language.\nGraphs compile to real AArch64 and x86-64 assembler.\n\nNative IDE: Rust + egui. No web interface, no telemetry.", "WireASM {} — визуальный язык поверх ассемблера.\nГрафы компилируются в настоящий ассемблер AArch64 и x86-64.\n\nСреда на Rust + egui. Без веба и телеметрии."),
    ("about.close", "Close", "Закрыть"),
    ("shortcuts.title", "Keyboard shortcuts", "Горячие клавиши"),
    ("shortcuts.text", "Ctrl/Cmd+N  new graph\nCtrl/Cmd+O  open\nCtrl/Cmd+S  save\nCtrl/Cmd+Shift+S  save as\nCtrl/Cmd+Z  undo\nCtrl/Cmd+Shift+Z  redo\nCtrl/Cmd+Enter  compile\nF5  build & run\nDelete  delete node\nF  fit graph\nCtrl/Cmd + wheel  zoom\nMiddle/right drag  pan", "Ctrl/Cmd+N  новый граф\nCtrl/Cmd+O  открыть\nCtrl/Cmd+S  сохранить\nCtrl/Cmd+Shift+S  сохранить как\nCtrl/Cmd+Z  отменить\nCtrl/Cmd+Shift+Z  вернуть\nCtrl/Cmd+Enter  компилировать\nF5  собрать и запустить\nDelete  удалить узел\nF  вписать граф\nCtrl/Cmd + колесо  масштаб\nСредняя/правая кнопка  панорама"),
    ("lang.system", "System language", "Как в системе"),
    ("target.not_buildable", "not runnable here", "не запускается здесь"),
    ("example.hello", "Hello world", "Hello world"),
    ("example.sum_loop", "Sum 1..10", "Сумма 1..10"),
    ("example.fibonacci", "Fibonacci", "Фибоначчи"),
    ("example.raw_asm", "Raw assembly", "Чистый ассемблер"),
    ("example.custom_nodes", "Custom nodes", "Свои узлы"),
    ("node.Entry", "Event Begin", "Начало"),
    ("node.Const", "Const", "Константа"),
    ("node.VarGet", "Get Var", "Читать переменную"),
    ("node.VarSet", "Set Var", "Записать переменную"),
    ("node.BinOp", "Binary Op", "Бинарная операция"),
    ("node.Cmp", "Compare", "Сравнение"),
    ("node.Branch", "Branch", "Ветвление"),
    ("node.PrintU64", "Print Number", "Печать числа"),
    ("node.PrintStr", "Print Text", "Печать текста"),
    ("node.Return", "Return", "Возврат"),
    ("node.RegGet", "Read Register", "Читать регистр"),
    ("node.RegSet", "Write Register", "Записать регистр"),
    ("node.Call", "Call", "Вызов"),
    ("node.Syscall", "Syscall", "Сисколл"),
    ("node.RawAsm", "Raw Assembly", "Сырой ассемблер"),
    ("node.Anchor", "Anchor", "Якорь"),
    ("node.Comment", "Comment", "Комментарий"),
];

pub fn t(lang: Lang, key: &str) -> &'static str {
    for (k, en, ru) in STRINGS {
        if *k == key {
            return match lang {
                Lang::En => en,
                Lang::Ru => ru,
            };
        }
    }
    // A missing translation must be obvious but must not allocate per frame.
    "?"
}
