# Журнал WireASM / WireStudio 0.2.0

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

### Финальная проверка и git

- Тесты: 12 из 12. Clippy по всему workspace без предупреждений. Release-сборка
  пересобрана после правок.
- Примеры снова прогнаны на `aarch64-macos`: `Hello from WireASM!`, `55`, `55`,
  `42`, `42`. `out/*.s` перегенерированы.
- Архив `releases/wirestudio-0.1.0-aarch64-macos.tar.gz` пересобран с новыми
  бинарниками (в git не попадает, `releases/` в `.gitignore`).
- Проверка редактора своих узлов: конфликт `kind`, лимиты 8×u64 / 1×u64 out,
  `exec`-выход без `exec`-входа и имя `exec` у `u64`-пина теперь отклоняются
  с понятным сообщением; подписи файловых диалогов переведены.
- Репозиторий: `main`, два коммита (`4d01223`, `8b0a4a9`), рабочее дерево
  чистое. Remote не добавлен, `gh` авторизован как `DOKODS`.

## 2026-11-09

### Публикация на GitHub

Репозиторий: https://github.com/DOKODS/wireasm (public). Ветка `main`, тег
`v0.1.0`, релиз: https://github.com/DOKODS/wireasm/releases/tag/v0.1.0.

Первый прогон CI упал, оба замечания исправлены коммитом `e215491`:

1. `cargo clippy --workspace --all-targets -- -D warnings` ругался на
   `Graph::default()` с присваиванием полей в тестовом хелпере `lib.rs`.
   Заменено на структурный литерал с `..Graph::default()`.
2. GitHub снял раннеры `macos-13` (retired) и `macos-14`
   (`configuration not supported`). В workflow заменены на `macos-15-intel`
   (Intel) и `macos-15` (Apple Silicon).

Тег переставлен на исправленный коммит, старый прогон отменён.

### Результат

- CI на `main`: success, все три платформы (Ubuntu, Windows, macOS arm64).
- Release на `v0.1.0`: success, 7 мин 43 с.
- Четыре архива в релизе: `aarch64-macos`, `x86_64-macos`, `x86_64-linux`,
  `x86_64-windows`; в каждом `wirestudio`, `wireasm`, README, LICENSE,
  CHANGELOG.
- Скачанный из релиза `aarch64-macos` проверен: `wireasm examples/hello.json
  --run` печатает `Hello from WireASM!`, exit 0.

## 2026-11-11

### Микроконтроллеры: AVR (ATmega328P)

Задача: добавить в WireASM/WireStudio программирование микроконтроллеров —
отдельные MCU-узлы, генерацию прошивки и загрузку в плату. Порядок: сначала
AVR (Arduino Uno/Nano), STM32 — следующим этапом.

Сделано:

- `Target::AvrAtmega328p` (`avr-atmega328p`, синонимы `avr`, `atmega328p`,
  `arduino-uno`, `arduino-nano`), точка входа `main`.
- 11 MCU-узлов: `McuPinMode`, `McuDigitalWrite/Read`, `McuDelayMs/Us`,
  `McuUartInit/Write/WriteStr`, `McuAdcRead`, `McuRegisterWrite/Read`.
- `codegen/avr.rs`: 16-битная арифметика в `r24:r25`, переменные и временные в
  `.bss`, строки в `.data`, встроенные `delay_ms`, `delay_us`, `uart_write_byte`,
  `uart_write_str`, `uart_print_u16`, `udiv16`; комментарии `;`.
- `PrintU64`/`PrintStr` на AVR выводят в UART0; пролог включает 9600 бод, если
  явного `McuUartInit` нет.
- Валидатор: пины 0–19, режимы out/in/pullup, каналы АЦП 0–7, baud > 0,
  адреса 0..0xFFFF, запрет `Call`/`Syscall`/своих узлов на AVR, предупреждение
  об усечении `Const` до 16 бит и о baud > 115200.
- `toolchain.rs`: поиск `avr-gcc`/`avr-objcopy`/`avr-size`/`avrdude` в PATH,
  Arduino15 (`~/Library/Arduino15/...`, `~/.arduino15/...`, `%LOCALAPPDATA%`)
  и PlatformIO; `build_avr`, `flash_avr` (`avrdude -c arduino`), `avr_ports`.
- CLI: `--build`, `--out-dir`, `--flash <port>`, `--board uno|nano`, `--ports`.
- WireStudio: меню «Микроконтроллер», группа MCU в палитре (появляется на
  AVR-цели), редакторы пина/режима/baud/канала/адреса, регистры `r0..r31`,
  окно «Плата и программатор» (порт, плата, папка), F6 — собрать прошивку,
  F7 — прошить, примеры `avr_blink` и `avr_uart` в меню «Примеры».
- Примеры `examples/avr_blink.json`, `examples/avr_uart_hello.json`.

Проверено:

- `cargo test --workspace` — 16 из 16. `cargo clippy --all-targets -- -D warnings`
  чисто. `cargo fmt --all`.
- `wireasm examples/avr_blink.json --build` → `.s/.elf/.hex/.bin`; `avr-size`:
  text 222, bss 132.
- `wireasm examples/avr_uart_hello.json` → `avr-gcc` линкует; `avr-size`:
  text 224, data 26, bss 132.
- `wireasm --ports` отрабатывает (на машине Arduino не подключена, портов нет).
- WireStudio запускается (процесс живёт, окно открывается).

Не проверено: реальная загрузка в плату и работа на кристалле — платы под рукой
нет. STM32 не начат: нужен `arm-none-eabi-gcc` (или `ld.lld`) и
`st-flash`/`dfu-util`, их на машине нет.

### Выпуск 0.2.0

- Версия поднята до `0.2.0`: `Cargo.toml`, `VERSION` в ядре, текст `--help`
  (теперь берётся из `VERSION`), сообщения валидатора, README, `docs/language.md`,
  план, журнал, `CHANGELOG.md` (`[0.2.0] — 2026-11-11`), имена архивов в
  `release.yml` и `releases/README.md`. Папка проекта переименована в
  `WIREASM_AARCH64_X86_0.2.0`.
- Локальная сборка `aarch64-macos`: `releases/wirestudio-0.2.0-aarch64-macos.tar.gz`.
  Проверено из архива: `wireasm --help` показывает 0.2.0, `hello.json --run`
  печатает `Hello from WireASM!` с кодом 0.
- Повторная проверка AVR: `avr_blink` — text 222, bss 132 (в прошлой записи
  было 218 — это до замены `tst` на `or`); `avr_uart_hello` — text 224,
  data 26, bss 132.
