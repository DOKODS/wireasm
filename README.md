# WireASM 0.3.0 + WireStudio

Визуальный язык поверх ассемблера: граф из узлов, как в Blueprint у Unreal,
компилируется в настоящий `.s` для AArch64, x86-64, ATmega328P и Cortex-M4,
в скетч Arduino для ESP32/ESP8266 или в исходник C/C++/C#.

```
граф JSON ──► wireasm-core ──► .s    ──► clang ─────────────► программа
                  │          ├─► .s    ──► arm-none-eabi-gcc ► .elf/.hex/.bin ► STM32
                  │          ├─► .ino  ──► arduino-cli ──────► ESP32/ESP8266
                  │          └─► .c/.cpp/.cs ─► cc / dotnet ─► программа
                  └── проверки, диагностика, статистика
```

**WireStudio** — нативная среда на Rust + egui. Никакого браузера, сервера и
телеметрии: один исполняемый файл для Linux, Windows и macOS.

## Что умеет

- Одиннадцать целей: `aarch64-macos`, `aarch64-linux`, `x86_64-linux`,
  `x86_64-macos`, `avr-atmega328p` (Arduino Uno/Nano), `stm32f411`
  (Cortex-M4), `esp32`, `esp8266`, плюс исходники `c`, `cpp`, `csharp`.
- 17 встроенных узлов для хоста и 11 узлов микроконтроллера: пины, задержки,
  UART, АЦП, запись и чтение регистров.
- Прошивка: `.elf/.hex/.bin` для AVR и STM32, скетч для ESP; сборка и загрузка
  из IDE (F6 — собрать, F7 — прошить).
- Классический редактор кода: вкладки, номера строк, подсветка синтаксиса,
  открытие/сохранение, сборка и запуск текущей цели.
- Свои узлы: пины, шаблоны ассемблера для обеих архитектур, свойства.
- Библиотеки узлов `*.wirelib.json`: импорт и экспорт.
- Импорт `.s` как заготовки своего узла.
- Интерфейс на русском и английском, по системной локали или вручную.
- Undo/redo, перетаскивание узлов, соединение пинов, панели Assembler,
  Problems и Console, сборка и запуск на текущей машине.
- Свой JSON-парсер и генераторы кода без внешних зависимостей в ядре и CLI.

## Быстрый старт

Нужны Rust (проверено на 1.99.1) и `clang` в `PATH`.

```bash
cd WIREASM_AARCH64_X86_0.2.0

cargo test --workspace
cargo build --release

# IDE
./target/release/wirestudio
./target/release/wirestudio examples/hello.json

# граф -> .s
./target/release/wireasm examples/sum_loop.json -o out/sum_loop.s

# только проверка, без записи файла
./target/release/wireasm examples/hello.json --check

# собрать и запустить на этой машине
./target/release/wireasm examples/fibonacci.json --run

# прошивка для Arduino: .elf/.hex/.bin
./target/release/wireasm examples/avr_blink.json --build --out-dir out

# залить в плату через загрузчик Arduino
./target/release/wireasm examples/avr_blink.json --flash /dev/cu.usbmodem14101 --board uno

# Cortex-M4: .elf/.hex/.bin через arm-none-eabi-gcc
./target/release/wireasm examples/stm32_blink.json --build

# залить в STM32 через st-link или DFU
./target/release/wireasm examples/stm32_blink.json --flash stlink

# ESP32/ESP8266: скетч Arduino и сборка arduino-cli
./target/release/wireasm examples/esp32_blink.json --build

# исходник C/C++/C# и запуск на хосте
./target/release/wireasm examples/c_hello.json --run
./target/release/wireasm examples/cpp_hello.json --run
./target/release/wireasm examples/csharp_hello.json --run

# список портов, похожих на Arduino
./target/release/wireasm --ports
```

CLI:

```
wireasm <graph.json> [options]
  -o <file>          куда писать результат (по умолчанию stdout)
  --target <t>       host asm | avr-atmega328p | stm32f411 | esp32 | esp8266
                     | c | cpp | csharp
  --check            только валидация
  --run              собрать и запустить программу (asm, C, C++, C#)
  --build            собрать прошивку/исходник (.elf/.hex/.bin/.exe)
  --out-dir <dir>    папка артефактов для --build (по умолчанию out)
  --flash <value>    порт для AVR/ESP или transport для STM32 (stlink|dfu)
  --board <name>     плата для AVR --flash: uno (по умолчанию) или nano
  --ports            показать последовательные порты
```

Для `--build`/`--flash` нужны тулчейны:

- AVR: `avr-gcc`, `avr-objcopy`, `avr-size`, `avrdude`.
- STM32: `arm-none-eabi-gcc` и `arm-none-eabi-objcopy`, для загрузки
  `st-flash` (st-link) или `dfu-util` (DFU).
- ESP: `arduino-cli` и ядро платы (`arduino-cli core install esp32:esp32`).
- C/C++: системный `cc`/`c++`; C#: .NET SDK (`dotnet`).

IDE от Arduino кладёт AVR-инструменты в
`~/Library/Arduino15/packages/arduino/tools/` на macOS,
`~/.arduino15/packages/arduino/tools/` на Linux и
`%LOCALAPPDATA%\Arduino15\packages\arduino\tools` на Windows; WireASM ищет их
там автоматически, а также в `PATH` и PlatformIO.

## WireStudio

| Действие | Как |
|---|---|
| Добавить узел | клик в палитре слева или перетащить на холст |
| Соединить пины | клик по выходу, затем по входу; Esc отменяет |
| Переместить узел | перетащить |
| Панорама | средняя или правая кнопка мыши |
| Масштаб | Ctrl/⌘ + колесо, кнопки в шапке |
| Инспектор | выделить узел, справа появляются свойства |
| Переменные | Edit → Variables… |
| Свой узел | Nodes → New custom node… |
| Компиляция | Ctrl/⌘+Enter |
| Сборка и запуск | F5 |
| Собрать прошивку | F6 или Microcontroller → Build firmware |
| Прошить плату | F7 или Microcontroller → Flash |
| Порт и плата | Microcontroller → Board settings… |
| Сменить цель | выпадающий список в шапке или Microcontroller |
| Граф / код | переключатель в шапке или View → Workspace |
| Открыть/сохранить код | Code → Open… / Save / Save as… |

Явная компиляция сохранённого графа кладёт рядом файл `<имя>.s`. Автокомпиляция
после правок обновляет панели, но не пишет файл. Экспорт — File → Export
assembler…, он же сохраняет папку экспорта в настройках.

Настройки (язык, последняя папка, папка экспорта, порт и плата AVR) лежат в:

- macOS: `~/Library/Application Support/WireStudio/settings.json`
- Linux: `~/.config/wirestudio/settings.json` (или `$XDG_CONFIG_HOME`)
- Windows: `%APPDATA%\WireStudio\settings.json`

## Микроконтроллеры и исходники

Одна и та же схема узлов работает на всех целях. Меняется только генератор:

| Цель | Что получается | Чем собирается |
|---|---|---|
| `avr-atmega328p` | `.s` для ATmega328P, `.elf/.hex/.bin` | `avr-gcc`, `avrdude` |
| `stm32f411` | Thumb-2 `.s` с векторами и `Reset_Handler` | `arm-none-eabi-gcc -nostdlib` |
| `esp32`, `esp8266` | скетч `setup()`/`loop()` | `arduino-cli` |
| `c`, `cpp`, `csharp` | `main()` с `goto`-метками | `cc` / `c++` / `dotnet` |

В палитре появляется группа «Микроконтроллер»: режим пина, цифровой
вход/выход, задержки в мс и мкс, UART (инициализация, байт, строка), АЦП и
запись/чтение регистров по адресу. Для STM32 пины задаются портом и номером
(`C13`), для AVR и ESP — одним номером.

```
Entry → Pin Mode (13, out) → Anchor → Digital Write 1 → Delay 500
                                        ↑                        │
                                        └──── Digital Write 0 ←──┘
```

Готовые графы: `examples/avr_blink.json`, `examples/avr_uart_hello.json`,
`examples/stm32_blink.json`, `examples/stm32_uart_hello.json`,
`examples/esp32_blink.json`, `examples/esp32_uart_hello.json`,
`examples/c_hello.json`, `examples/cpp_hello.json`,
`examples/csharp_hello.json`.

AVR работает с 16-битными значениями в `r24:r25`, STM32 и ESP — с 32-битными;
валидатор предупреждает об усечении. `Call`, `Syscall` и свои узлы на MCU и
исходниках недоступны: у микроконтроллера нет ОС и хостового ABI. Исключение —
`Call` в C/C++/C#: это обычный вызов функции из стандартной библиотеки.

STM32 стартует от внутреннего HSI 16 МГц, USART2 выводит на PA2 (TX);
`PrintU64`/`PrintStr` идут туда же. Прошивка — `out/<имя>.elf`, `.hex`, `.bin`.
Для загрузки нужен `st-flash` или `dfu-util`; транспорт выбирается в
Microcontroller → Board settings.

## Редактор кода

Кнопка «Код» в шапке открывает вторую рабочую область — обычный текстовый
редактор с вкладками, номерами строк и подсветкой под C, C++, C#, Rust,
ассемблер и JSON. Файл можно открыть (Code → Open…), создать или сохранить;
сгенерированный при компиляции исходник открывается отдельной вкладкой и
обновляется после каждой сборки. Оттуда же работают Build (F6) и Run (F5) для
текущей цели.

Горячие клавиши: Ctrl/⌘+S — сохранить, Ctrl/⌘+Enter — компилировать граф,
F5 — собрать и запустить, F6 — собрать прошивку.

## Свои узлы

Свой узел — это пины плюс два шаблона ассемблера. В редакторе задаются `kind`,
название, категория, описание, входы/выходы и шаблоны AArch64 и x86-64.

Контракт шаблона:

| Место | AArch64 | x86-64 |
|---|---|---|
| входы `{in0}..{in7}` | `x0..x7` | `rdi, rsi, rdx, rcx, r8, r9, r10, r11` |
| результат `{out}` | `x0` | `rax` |
| `{a}`, `{b}` | первые два входа | первые два входа |
| `{label}` | уникальный префикс меток | уникальный префикс меток |
| `{prop:key}` | свойство экземпляра | свойство экземпляра |

Пример:

```json
{
  "kind": "lib_double",
  "title": "Double",
  "inputs":  [{ "name": "value", "type": "u64" }],
  "outputs": [{ "name": "result", "type": "u64" }],
  "asm": {
    "aarch64": "lsl {out}, {in0}, #1",
    "x86_64": "mov rax, rdi\nshl rax, 1"
  }
}
```

Ограничения 0.3.0: до 8 входов `u64`, не больше одного выхода `u64`, не больше
одного `exec`-входа и одного `exec`-выхода; `exec`-выход требует `exec`-входа.
Шаблонов под AVR у своих узлов пока нет.

### Библиотеки

`File → Import node library…` читает файл вида:

```json
{
  "format": "wireasm-library",
  "version": "1",
  "name": "WireASM math",
  "nodes": [ /* определения узлов */ ]
}
```

Импорт заменяет узлы с тем же `kind` и добавляет новые. Экспорт берёт все свои
узлы текущего графа. Готовый пример — `examples/wirelib_math.wirelib.json`.

`File → Import .s as node…` создаёт узел из существующего ассемблерного файла.
Пины по умолчанию — `exec` + `value: u64` на входе и `exec` + `result: u64` на
выходе; их нужно поправить под реальный код.

## Язык и генерация

Полное описание формата графа, пинов, узлов, циклов и валидатора —
в [docs/language.md](docs/language.md).

Коротко:

- `exec` — белый провод выполнения; вход может принимать несколько проводов.
- `u64` — голубой провод данных; вход принимает один провод.
- Данные вычисляются лениво, в точке использования.
- Обратный `exec`-провод разрешён только в `Anchor` — так задаются циклы.
- `RawAsm` не переводится между синтаксисами: один узел рассчитан на одну
  архитектуру.

## Структура проекта

```
crates/wireasm-core/   модель, JSON, валидатор, кодогенераторы, toolchain
crates/wireasm-cli/    CLI wireasm
crates/wirestudio/     нативная IDE на eframe/egui
examples/              графы и библиотека
out/                   сгенерированные .s для примеров
docs/language.md       справочник языка
releases/              локальные архивы и правила релиза
```

## Проверено

На macOS AArch64 (`rustc 1.99.1`, Apple clang, avr-gcc 7.3.0,
arm-none-eabi-gcc 16.2.0, arduino-cli 1.6.0):

- `cargo test --workspace` — 19 тестов, все проходят.
- `cargo clippy --all-targets -- -D warnings` — без предупреждений.
- Пять хост-примеров собираются и запускаются: `Hello from WireASM!`, `55`,
  `55`, `42`, `42`.
- Ассемблер для `aarch64-linux`, `x86_64-linux` и `x86_64-macos` проходит
  `clang -c` с соответствующей тройкой.
- `examples/avr_blink.json` и `examples/avr_uart_hello.json` собираются
  `avr-gcc -mmcu=atmega328p -Os`; `avr-size`: blink — text 218 / bss 132,
  uart — text 224 / data 26 / bss 132.
- `examples/stm32_blink.json` — text 244 / bss 260, `stm32_uart_hello` —
  text 411 / bss 260 (`arm-none-eabi-gcc -mcpu=cortex-m4 -mthumb -nostdlib`).
- `examples/c_hello.json --run` и `examples/cpp_hello.json --run` печатают
  `Hello from WireASM C!` и `n = 5`.
- ESP собирается в скетч, но ядра `esp32:esp32`/`esp8266:esp8266` не
  установлены — `--build` честно останавливается с командой установки.
- C# требует .NET SDK, STM32-загрузка — `st-flash`/`dfu-util`; на этой машине
  их нет, поэтому проверены только диагностические сообщения.
- `wireasm --build` и `--ports` проверены; реальная загрузка в плату не
  проверялась — под рукой не было плат.

Linux и Windows бинарники собираются в GitHub Actions; на этой машине они не
проверялись. macOS-приложение не подписано и не нотаризовано.

## Релизы

Готовые сборки — [WireASM 0.3.0](https://github.com/DOKODS/wireasm/releases/tag/v0.3.0).

`.github/workflows/release.yml` по тегу `v*` собирает архивы:

- `wirestudio-0.3.0-aarch64-macos.tar.gz`
- `wirestudio-0.3.0-x86_64-macos.tar.gz`
- `wirestudio-0.3.0-x86_64-linux.tar.gz`
- `wirestudio-0.3.0-x86_64-windows.zip`

В каждом архиве `wirestudio`, `wireasm`, README, лицензия и changelog.
Локальные архивы кладутся в `releases/`, подробности — в
[releases/README.md](releases/README.md).

## Лицензия

MIT, см. [LICENSE](LICENSE).

---

**English.** WireASM is a visual assembler language: Blueprint-style graphs
compile to real AArch64, x86-64, ATmega328P or Cortex-M4 assembly, to an Arduino
sketch for ESP32/ESP8266, or to C/C++/C# source. WireStudio is a native Rust/egui
IDE for Linux, Windows and macOS with a built-in code editor, MCU nodes,
firmware build and flash for AVR/STM32/ESP, custom assembler nodes, node
libraries, and English/Russian UI. Build with `cargo build --release`; see the
commands above.
