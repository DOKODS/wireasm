# Журнал WireASM / WireStudio 0.1.0

## 2026-11-07

### Поиск

Проверены GitHub и документация похожих проектов:

- `p-ranav/box` — текстовый визуальный язык по мотивам Blueprint, свой
  интерпретатор, не ассемблер.
- `xodio/xod` — dataflow для микроконтроллеров, генерация C++.
- `Dman95/SASM`, Visual MASM — IDE для ассемблера без графов.
- `RobinKa/netprints` — Blueprint-подобный язык для .NET.

Полного совпадения («граф Blueprint → готовый `.s`») не нашлось. Решено делать
свой компилятор.

### Среда

- Rust 1.99.1 поставлен через Homebrew, `cargo` в `/opt/homebrew/bin`.
- `clang` — `/usr/bin/clang`.
- Хост: macOS, `aarch64`.

### Сделано

- Workspace из трёх крейтов: `wireasm-core`, `wireasm-cli`, `wirestudio`.
- Свой JSON-парсер и писатель, без serde.
- Модель: Entry, Branch, Anchor, Return, Const, VarGet, VarSet, BinOp, Cmp,
  PrintU64, PrintStr, Call, Syscall, RegGet, RegSet, RawAsm, Comment.
- Валидатор: типы, обязательные пины, единственность Entry, циклы данных,
  циклы exec только через Anchor, достижимость.
- Кодогенераторы AArch64 и x86-64 с сисколлами и рантаймом `print_u64`.
- CLI: `--check`, `-o`, `--target`, `--run`.
- IDE: HTTP-сервер на `127.0.0.1`, API `/api/health`, `/api/examples`,
  `/api/example`, `/api/compile`, `/api/build`; фронтенд на чистом JS.

### Исправленные ошибки

1. Метки строк начинались с `S`; Apple clang принимал `S0@PAGEOFF` за ошибку.
   Переименованы в `LstrN`.
2. AArch64-слоты кадра адресовались через `[x29, #-offset]`; при смещении
   больше ±256 ассемблер падал. Переведены на положительные `[sp, #N]`.
3. `print_u64` печатал перевод строки перед числом (`\n55`). Буфер перестроен:
   цифры идут первыми, `\n` добавляется после них.
4. В `wirestudio` были ошибки сборки: `path.as_str()` на `&str` (нестабильный
   метод), лишние `;` в ветках `match`, лишние `mut`. Исправлено.
5. CLI в справке ссылался на несуществующие `*.wire.json`; заменено на
   `examples/*.json`.
6. Мелочи clippy: `as_bytes().len()`, `Default` для `Emitter`, `&PathBuf` →
   `&Path`, вложенный `if` в валидаторе.
7. `Call` с `func = print_u64` не находил рантайм: `uses_print_u64` смотрел
   только на узлы `PrintU64`. Теперь рантайм добавляется и для такого `Call`;
   есть тест `call_to_print_u64_emits_runtime`.

### Проверки

```bash
cargo test --release
# 9 passed

cargo clippy --workspace --release
# без предупреждений

cargo build --release
```

Запуск примеров на хосте `aarch64-macos`:

```text
hello      Hello from WireASM!   exit 0
sum_loop   55                    exit 0
fibonacci  55                    exit 0
raw_asm    42                    exit 0
```

Кросс-ассемблирование: 4 графа × 4 цели = 16 файлов, все собираются
`clang -c`:

```text
assembled=16 failed=0
```

Дополнительно x86_64-macos-бинарники запущены через Rosetta:

```text
hello      Hello from WireASM!   status 0
sum_loop   55                    status 0
fibonacci  55                    status 0
```

`raw_asm` на x86 не запускался: его текст `add {a}, {a}, {b}` — синтаксис
AArch64. Это ограничение задокументировано.

### IDE

Сервер запущен:

```bash
./target/release/wirestudio --no-open --port 8787
```

Проверено через браузер:

- страница открывается, ошибок в консоли нет;
- пример `Sum 1..10` загружается, 21 узел, 21 провод, 2 переменные;
- Compile: `ok · 127 lines`, Problems: пусто;
- Build & Run: `exit status: Some(0)`, stdout `55`;
- добавление узла и соединение пинов работают (число проводов выросло с 1 до 2);
- при дублирующем Entry панель Problems показывает ошибку и два предупреждения;
- переключение цели на `x86_64-linux` меняет шапку ассемблера на `_start`;
- пример `hello` после перезагрузки: Compile `ok · 53 lines`, Build & Run
  `ok · 53 lines · exit 0`, stdout `Hello from WireASM!`.

### Состояние

Работает на `aarch64-macos`. Linux-цели проверены только ассемблированием.
Документация: `README.md`, `docs/language.md`, `plan.md`, этот журнал.

## 2026-11-08

### Переход на нативную IDE

Веб-интерфейс и HTTP-сервер удалены полностью: `main.rs` переписан под
eframe/egui 0.37 с рендерером glow, `crates/wirestudio/assets/` удалён, порт
8787 больше никем не слушается. Внутри приложения теперь всё: палитра, холст,
инспектор, переменные, свои узлы, ASM/Problems/Console, файловые диалоги.

Причины: требование «IDE на Rust, без веба» и желание публиковать один
исполняемый файл на три ОС.

### Свои узлы и библиотеки

- `CustomNodeDef` / `CustomPin` / `Library` в `wireasm-core`.
- Шаблоны для AArch64 и x86-64, плейсхолдеры `{in0}..{in7}`, `{a}`, `{b}`,
  `{out}`, `{label}`, `{prop:key}`.
- Валидация: до 8 входов `u64`, один выход `u64`, один `exec`-вход и один
  `exec`-выход, `exec`-выход только вместе с `exec`-входом.
- Кодогенераторы вставляют шаблон в общий поток, слоты и временные ячейки
  в кадре не пересекаются.
- Библиотека `*.wirelib.json`: импорт заменяет совпадающие `kind`, экспорт
  берёт все свои узлы графа.
- Импорт `.s` создаёт заготовку узла с пинами по умолчанию.
- Примеры: `examples/custom_nodes.json`, `examples/wirelib_math.wirelib.json`.

### Локализация

`i18n.rs` — таблица en/ru примерно на 175 строк. Язык берётся из
`sys-locale`, ручной выбор хранится в настройках. Для кириллицы добавлен
системный шрифт-фолбэк (Arial Unicode / DejaVu / Noto / Segoe UI).

### IDE

- Меню File/Edit/Nodes/View/Language/Help, панель инструментов с целью.
- Холст: сетка, безье-провода, перетаскивание, панорама, зум, вписать граф,
  соединение пинов кликом, drag-and-drop из палитры.
- Инспектор: свойства всех встроенных узлов, свои `{prop:key}`, удаление.
- Переменные: добавление, удаление, переименование с обновлением ссылок.
- Редактор своих узлов: метаданные, пины, оба шаблона, проверка и удаление.
- Undo/redo, защита от закрытия с несохранёнными изменениями.
- Compile пишет `<имя>.s` рядом с сохранённым графом; Build & Run собирает
  через clang во временной папке с лимитом 5 с.

### Ошибки, найденные при сборке

1. В egui 0.37 панели объединены в `Panel::top/bottom/left/right`; старые
   `TopBottomPanel`/`SidePanel` не экспортируются. Переписано.
2. `Window::default_size` принимает `Vec2`, а не число.
3. В canvas перепутаны `Pos2`/`Vec2` при захвате узла и лишний `.to_pos2()`.
4. `Context::wants_keyboard_input` переименован в `egui_wants_keyboard_input`.
5. Двойное заимствование `self` при перетаскивании узла: заменено на индекс.
6. `match` по коду ответа в диалоге закрытия был неисчерпывающим.
7. Clippy: индексация в циклах x86-64 и редакторе пинов, `while let`,
   лишние `format!`, `&str`- deref.

### Проверки

```bash
cargo test --workspace
# 12 passed

cargo clippy --workspace --release
# без предупреждений

cargo build --release --workspace
# wirestudio 5.6 MB, wireasm 0.6 MB
```

Запуск примеров на `aarch64-macos`:

```text
hello         Hello from WireASM!   exit 0
sum_loop      55                    exit 0
fibonacci     55                    exit 0
raw_asm       42                    exit 0
custom_nodes  42                    exit 0
```

`wirestudio examples/hello.json` запускается и остаётся живым (smoke-тест,
процесс убит вручную через 4 с). Визуальная проверка окна на этой машине
не автоматизирована.

Кросс-ассемблирование `clang -c` для `aarch64-linux`, `x86_64-linux`,
`x86_64-macos` — 15 файлов, ошибок нет.

### Релизная обвязка

- `.gitignore`, MIT `LICENSE`, `CHANGELOG.md`.
- `.github/workflows/ci.yml`: тесты, clippy, проверка примеров на Ubuntu,
  Windows и macOS.
- `.github/workflows/release.yml`: матрица из четырёх платформ, архивы
  `.tar.gz`/`.zip`, прикрепление к GitHub Release по тегу `v*`.
- `releases/README.md` с именами архивов и предупреждением о карантине macOS.

### Состояние

Готово к публикации. Локально проверен только `aarch64-macos`; Linux и Windows
собираются в CI. macOS-сборка не подписана.
