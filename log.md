# Журнал WireASM / WireStudio

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

Публикация:

- Коммит `8674934`, тег `v0.2.0`, релиз
  https://github.com/DOKODS/wireasm/releases/tag/v0.2.0.
- CI на `main`: success. Release на `v0.2.0`: success, четыре архива —
  `aarch64-macos` (3.0 МБ), `x86_64-macos` (3.4 МБ), `x86_64-linux` (4.3 МБ),
  `x86_64-windows` (3.7 МБ).
- Скачанный из релиза `aarch64-macos`: `wireasm --help` печатает 0.2.0,
  `examples/hello.json --run` — `Hello from WireASM!`, код 0.

## 2026-11-12 — 0.3.0

### Цели

- `Target`: `Stm32F411`, `Esp32`, `Esp8266`, `CLang`, `Cpp`, `CSharp`.
  Синонимы, `label()`, `family()`, `is_stm32/is_esp/is_arduino/is_source`,
  расширенные `is_mcu`, `entry_symbol` и `source_extension`.
- Старые пять целей не тронуты: поведение хоста и AVR прежнее.

### Кодогенераторы

- `codegen/stm32.rs`: полный образ Cortex-M4 — таблица векторов,
  `Reset_Handler`, `Default_Handler`, `main`, SysTick-задержки, USART2,
  GPIO, АЦП, `udiv32`; `.thumb_func` на каждой ветке.
- `codegen/arduino.rs`: `setup()`/`loop()`, `pinMode`, `digitalWrite`,
  `delay`, `Serial`, `analogRead`; переменные как `static uint32_t`.
- `codegen/c_lang.rs`: C, C++, C# — общий `main()` с метками и `goto`;
  для C# класс `Program` и `Main()`.
- Диспетчер, шапка файла, префиксы комментариев и `VERSION = "0.3.0"`.

### Тулчейн и CLI

- `arm-none-eabi-gcc -nostdlib` с линкерным скриптом STM32F411
  (flash 0x08000000/512K, RAM 0x20000000/128K); `st-flash`/`dfu-util`
  для загрузки.
- `arduino-cli`: FQBN для ESP32/ESP8266, проверка установленного ядра,
  `compile` и `upload`.
- `build_source` для C/C++/C#; `--run` для хостовых исходников.
- CLI: цели в `--target`, расширение результата по цели, `--build`,
  `--flash stlink|dfu|<порт>`, обновлённая справка.

### Валидатор

- Семейные проверки пинов: AVR 0–19, STM32 порт A–H и пин 0–15,
  ESP32 0–39, ESP8266 0–16; каналы АЦП и регистры по семействам.
- 32-битные адреса и предупреждение об усечении `Const` для STM32/ESP.
- `Call`/`Syscall`/свои узлы запрещены на MCU; `Call` разрешён в C/C++/C#.

### WireStudio

- `editor.rs`: вкладки, номера строк, свой лексер и подсветка под C, C++,
  C#, Rust, asm, JSON; открытие/сохранение, обновление сгенерированной
  вкладки, сборка и запуск.
- `Workspace::{Graph, Code}`, переключатель в шапке и меню View.
- Меню целей, F6/F7 для AVR/STM32/ESP, окно платы по цели, транспорт STM32.
- Инспектор STM32: порт/пин/режим, регистры `r0..r12/sp/lr/pc`.
- i18n: цели, меню кода, сообщения сборки, новые примеры.

### Примеры и тесты

- `stm32_blink`, `stm32_uart_hello`, `esp32_blink`, `esp32_uart_hello`,
  `c_hello`, `cpp_hello`, `csharp_hello`; добавлены в меню «Примеры».
- Тесты: STM32 Thumb-2, ESP-скетч, три исходника. Всего 19, все зелёные.
- Найден и исправлен баг: `PrintU64` на STM32 звал `udiv32`, но helper
  не эмитился — теперь `number` включает `divide`.

### Проверено

- `cargo test --workspace` — 19/19; `cargo check` по всему workspace.
- `wireasm examples/stm32_blink.json --build` — text 244, bss 260;
  `stm32_uart_hello` — text 411, bss 260.
- `wireasm examples/c_hello.json --run` и `cpp_hello.json --run` печатают
  `Hello from WireASM C!` и `n = 5`, код 0.
- `wireasm examples/esp32_blink.json --build` — понятная ошибка об
  отсутствующем ядре `esp32:esp32`.
- `csharp_hello --run` — понятная ошибка об отсутствующем `dotnet`.
- Загрузка на платы не проверялась: плат, `st-flash`, `dfu-util` нет.

### Выпуск 0.3.0

- Версия `0.3.0` в `Cargo.toml`, `VERSION` ядра, `--help`, README,
  `docs/language.md`, плане, журнале, `CHANGELOG.md` (`[0.3.0] — 2026-11-12`),
  именах архивов `release.yml` и `releases/README.md`.
- Локальный архив `releases/wirestudio-0.3.0-aarch64-macos.tar.gz` (3.0 МБ).
  Из архива: `wireasm --help` печатает 0.3.0, `hello.json --run` —
  `Hello from WireASM!`, код 0; `wirestudio` запускается и держит окно.
- `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` — чисто;
  `cargo test --workspace` — 19 из 19.
- Коммит `24011ed`, тег `v0.3.0`, релиз
  https://github.com/DOKODS/wireasm/releases/tag/v0.3.0.
- CI на `main` — success (1m41s). Release на `v0.3.0` — success (6m25s),
  четыре архива: `aarch64-macos` (3.2 МБ), `x86_64-macos` (3.5 МБ),
  `x86_64-linux` (4.5 МБ), `x86_64-windows` (3.9 МБ).
- Скачанный из релиза `aarch64-macos`: `wireasm --help` — 0.3.0,
  `hello.json --run` — `Hello from WireASM!`, код 0.
- Папка переименована в `WIREASM_AARCH64_X86_0.3.0`, индекс проектов
  обновлён.

### Дальше

0.4.0 — эмуляция схем: MNA-солвер, диоды и переходные процессы,
цифровые элементы, инструменты, co-simulation прошивки. Начата отдельная
крейта `wireasm-sim`.

## 2026-11-12 — 0.4.0, эмуляция схем

### Что сделано

- Новая крейт `wireasm-sim` (только `wireasm-core`, без внешних зависимостей):
  - `netlist.rs` — компоненты и выводы, JSON-вход и JSON-вывод.
  - `linalg.rs` — плотное LU-разложение для вещественных и комплексных матриц.
  - `solver.rs` — MNA, Ньютон–Рафсон с ограничением шага pn-перехода
    (0.25 В), переходный процесс по обратному Эйлеру с начальными
    условиями (конденсатор — источник `ic`, катушка — источник тока `ic`),
    комплексная АЧХ.
  - `mcu.rs` — виртуальный МК: исполняет граф WireASM в масштабе времени
    схемы, отдаёт состояние выводов и перехватывает UART.
  - `trace.rs` — осциллограммы: пик, минимум, среднее, RMS, фронт, CSV.
  - `schematic.rs` — модель схемы: части, провода, цепи (union-find),
    преобразование в нетлист, привязки выводов МК, JSON.
- CLI `wiresim`: расчёт DC/TRAN/AC для нетлиста или схемы, `--mcu` с
  графом прошивки, `--csv` для выгрузки.
- WireStudio: третья рабочая область «Схема» — палитра компонентов, холст с
  протяжкой проводов между выводами, инспектор свойств, панель анализа,
  осциллограф с легендой, щупы, привязка выводов виртуального МК, сохранение
  схемы, экспорт нетлиста, определение типа файла при открытии.

### Исправления в решателе

- Переходный процесс стартует из начальных условий, а не делает
  «фантомный» первый шаг: кривая зарядки RC совпадает с аналитической.
- Ограничение pn-перехода не даёт Ньютону разойтись на диоде.
- Светодиод проводит ток при 2 В: верхний кламп экспоненты поднят до `exp(80)`.
- Модель диода по умолчанию: `is=1e-14`, `n=1.0`, `rs=0.1`.
- Виртуальный МК продолжает работу после узла задержки, а не входит в него
  заново; `dc()` двигает МК до `t=0`.

### Примеры

- Схемы: `examples/circuit_divider.json`, `circuit_rc_lowpass.json`,
  `circuit_led.json`, `circuit_mcu_led.json`.
- Нетлисты: `examples/sim_voltage_divider.json`, `sim_rc_lowpass.json`,
  `sim_halfwave.json`, `sim_led_resistor.json`, `sim_logic_and.json`,
  `sim_mcu_led.json`.

### Проверено

- `cargo test --workspace` — 47 тестов (20 core, 21 sim, 6 wirestudio).
- `cargo clippy --all-targets -- -D warnings` — чисто; `cargo fmt --all`.
- Делитель: 3.836735 В. Светодиод: 2.048 В, 13.4 мА. RC-срез: −2.99 дБ на
  158 Гц. Выпрямитель: пик 4.31 В. Логика AND: выход высокий.
- `wiresim examples/circuit_mcu_led.json --mcu examples/stm32_blink.json`
  даёт меандр 0…3.14 В на C13 — привязки и питание взяты из схемы.
- Сравнение с ngspice 47 (`tools/bench_ngspice.py`, `tools/compare_ngspice.py`):
  делитель 3.836735 В в обоих; лестница из 200 R — 0.02487562 В против
  0.024875 В; АЧХ RC-фильтра расходится до 5·10⁻⁸ дБ; переходный процесс RC —
  до 0.96 мВ (у нас обратный Эйлер, у ngspice трапеция с адаптивным шагом).
  Скорость: 200 000 точек переходного процесса за 0.099 с против 0.22 с у
  ngspice, АЧХ 1000 точек за 0.18 мс против 10 мс.

### Выпуск 0.4.0

- Версия `0.4.0` в `Cargo.toml`, `VERSION` ядра, `--help`, README,
  `docs/language.md`, плане, журнале, `CHANGELOG.md`, именах архивов
  `release.yml` и `releases/README.md`.
- Локальный архив `releases/wirestudio-0.4.0-aarch64-macos.tar.gz` (3.5 МБ).
- Коммиты: `20d41b0` (режим схемы), `447b1f1` (бенчмарк и сравнение с
  ngspice), `ae10207` (исправление CI). Тег `v0.4.0` — на `ae10207`.
- CI на `main` — success: ubuntu 59 с, macos 1 мин 24 с, windows 1 мин 58 с.
  Первый прогон падал: цикл проверки примеров считал схемы графами; теперь
  он их пропускает и вместо этого считает каждую схему через `wiresim`.
- Release на `v0.4.0` — success, четыре архива: `aarch64-macos` (3.5 МБ),
  `x86_64-macos` (3.8 МБ), `x86_64-linux` (4.7 МБ), `x86_64-windows` (4.1 МБ).
- Скачанный из релиза `aarch64-macos`: `wireasm --help` печатает 0.4.0,
  `hello.json --run` — `Hello from WireASM!`, `wiresim examples/circuit_divider.json`
  — 3.836735 В, `wiresim` со схемой и графом мигания — меандр 0…3.14 В.
- Папка переименована в `WIREASM_AARCH64_X86_0.4.0`, индекс проектов
  обновлён.

## 2026-11-12 — 0.4.1, ГОСТ-УГО и элементная база

### Сделано

- УГО по ГОСТ для всех элементов схемы: 2.722 (резистор прямоугольником),
  2.723 (катушка тремя дугами), 2.725 (конденсатор двумя пластинами),
  2.727 (диод и светодиод), 2.730 и 2.735 (источники), 2.741 (транзисторы),
  2.742 (земля), 2.759 (ключи), 2.610 (логический элемент с обозначением
  функции), 2.318 (микросхема корпусом с выводами по краям).
  Модуль `crates/wirestudio/src/symbols.rs`.
- Каталог компонентов `wireasm_sim::library`: 133 элемента в семействах
  Multisim — базовые, TTL 74xx (50), TTL Шоттки (26), CMOS 4000 (18),
  ECL 10xxx (20), аналоговые (3), дискретные (7). У каждого корпуса своя
  распиновка и питание.
- Симуляция логических ИС: корпус разворачивается в юниты, тактовые
  элементы держат состояние между фронтами. Модуль `wireasm_sim::ic`.
- Импорт и экспорт библиотек компонентов: File → Import/Export component
  library…, JSON с набором элементов.
- Палитра с группами и поиском по названию и номеру.
- Примеры: `circuit_ttl_nand`, `circuit_ttl_counter`,
  `circuit_cmos_oscillator`.

### Проверено

- `cargo test --workspace` — 60 тестов; clippy чисто.
- Счётчик 74161 с тактом 1 кГц считает: q0, затем q1.
- Генератор на 4093 с RC 10 кОм/1 нФ: 2001 переключение за 2 мс.
- 7400 как инвертор: высокие входы дают низкий выход.
- Библиотека компонентов переживает круг JSON.

### Выпуск 0.4.1

- Версия `0.4.1` в `Cargo.toml`, `VERSION` ядра, README, `docs/language.md`,
  плане, журнале, `CHANGELOG.md`, именах архивов `release.yml` и
  `releases/README.md`.
- Локальный архив `releases/wirestudio-0.4.1-aarch64-macos.tar.gz` (3.6 МБ).
- Коммит `37b706d`, тег `v0.4.1`, релиз
  https://github.com/DOKODS/wireasm/releases/tag/v0.4.1.
- CI на `main` — success: ubuntu 36 с, macos 53 с, windows 1 мин 39 с.
  Первый прогон отменил macOS-задачу: раннер был занят сборкой релиза;
  перезапуск прошёл чисто.
- Release на `v0.4.1` — success, четыре архива: `aarch64-macos` (3.5 МБ),
  `x86_64-macos` (3.9 МБ), `x86_64-linux` (4.8 МБ), `x86_64-windows` (4.2 МБ).
- Скачанный из релиза `aarch64-macos`: `wireasm --help` печатает 0.4.1,
  `wiresim examples/circuit_divider.json` — 3.836735 В, счётчик 74161
  считает q0 и q1.
- Папка переименована в `WIREASM_AARCH64_X86_0.4.1`, индекс проектов
  обновлён.

## 2026-11-12 — 0.4.2, оформление по Multisim

### Сделано

- Редактор схем переделан по образцу Multisim: светлый лист с точечной
  сеткой, красные провода с отводами под 90°, чёрные корпуса элементов,
  синие подписи (обозначение сверху, номинал снизу), номера узлов на
  цепях, точки соединений там, где сходятся три и более провода.
  Модуль `crates/wirestudio/src/sheet.rs`.
- Виртуальные приборы как в Multisim: XSC1 (осциллограф, 4 канала),
  XMM1 (мультиметр), XFG1 (генератор функций), XBP1 (плоттер АЧХ),
  XLA1 (логический анализатор), XWM1 (ваттметр), XFC1 (частотомер).
  Прибор ставится на схему компонентом, панель открывается после расчёта,
  двойным щелчком или клавишей Enter.
- Решатель пишет измерения приборов в осциллограмму: `Trace::instruments`.
- Редактор кода: ширина по окну вместо жёстких 1200 px, метрики шрифта
  из egui вместо прикидки, перенос строк выключен, номера строк в своей
  колонке без второй полосы прокрутки. Это чинит развёрнутое окно на
  Windows.
- README переписан: вместо рекламного списка — описание того, что есть.

### Проверено

- `cargo test --workspace` — 61 тест; clippy чисто.
- Прибор XSC1 на схеме `circuit_scope_demo` показывает вход и выход RC-фильтра
  на двух каналах.
- Редактор кода занимает всё окно, номера строк не съезжают.

### Дальше

0.5.0 — SPICE-модели (`.model`), транзисторы, интерактивные пробники,
модели нагрузки, Фурье-анализ.
