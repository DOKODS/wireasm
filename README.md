# WireASM 0.2.0 + WireStudio

Визуальный язык поверх ассемблера: граф из узлов, как в Blueprint у Unreal,
компилируется в настоящий `.s` для AArch64, x86-64 или ATmega328P.

```
граф JSON ──► wireasm-core ──► .s ──► clang ──► исполняемый файл
                  │                └─► avr-gcc ─► .elf/.hex/.bin ─► плата
                  └── проверки, диагностика, статистика
```

**WireStudio** — нативная среда на Rust + egui. Никакого браузера, сервера и
телеметрии: один исполняемый файл для Linux, Windows и macOS.

## Что умеет

- Пять целей: `aarch64-macos`, `aarch64-linux`, `x86_64-linux`, `x86_64-macos`,
  `avr-atmega328p` (Arduino Uno/Nano).
- 17 встроенных узлов для хоста и 11 узлов микроконтроллера: пины, задержки,
  UART, АЦП, запись и чтение регистров ATmega328P.
- Прошивка: сборка `.elf/.hex/.bin` через `avr-gcc` и загрузка через `avrdude`
  прямо из IDE (F6 — собрать, F7 — прошить).
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

# список портов, похожих на Arduino
./target/release/wireasm --ports
```

CLI:

```
wireasm <graph.json> [options]
  -o <file.s>        куда писать ассемблер (по умолчанию stdout)
  --target <t>       переопределить цель из графа
  --check            только валидация
  --run              собрать через clang и запустить (лимит 5 с)
  --build            собрать AVR-прошивку (.elf/.hex/.bin) через avr-gcc
  --out-dir <dir>    папка артефактов для --build (по умолчанию out)
  --flash <port>     собрать и залить .hex через avrdude
  --board <name>     плата для --flash: uno (по умолчанию) или nano
  --ports            показать последовательные порты
```

Для `--build`/`--flash` нужен AVR-тулчейн: `avr-gcc`, `avr-objcopy`,
`avr-size` и (для загрузки) `avrdude`. IDE от Arduino кладёт их в
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
| Собрать AVR-прошивку | F6 или Microcontroller → Build firmware |
| Прошить плату | F7 или Microcontroller → Flash |
| Порт и плата | Microcontroller → Board settings… |

Явная компиляция сохранённого графа кладёт рядом файл `<имя>.s`. Автокомпиляция
после правок обновляет панели, но не пишет файл. Экспорт — File → Export
assembler…, он же сохраняет папку экспорта в настройках.

Настройки (язык, последняя папка, папка экспорта, порт и плата AVR) лежат в:

- macOS: `~/Library/Application Support/WireStudio/settings.json`
- Linux: `~/.config/wirestudio/settings.json` (или `$XDG_CONFIG_HOME`)
- Windows: `%APPDATA%\WireStudio\settings.json`

## Микроконтроллеры

Цель `avr-atmega328p` собирает граф в прошивку Arduino Uno/Nano. В палитре
появляется группа «Микроконтроллер»: режим пина, цифровой вход/выход,
задержки в мс и мкс, UART (инициализация, байт, строка), АЦП A0–A7 и
запись/чтение регистров ATmega328P по адресу.

```
Entry → Pin Mode (13, out) → Anchor → Digital Write 1 → Delay 500
                                        ↑                        │
                                        └──── Digital Write 0 ←──┘
```

Готовые графы: `examples/avr_blink.json`, `examples/avr_uart_hello.json`.
UART по умолчанию 9600 бод, 8N1; `PrintU64`/`PrintStr` на AVR тоже пишут в
UART. Значения 16-битные, переменные — в SRAM. `Call`, `Syscall` и свои узлы
на AVR не поддерживаются: у микроконтроллера нет ОС и хостового ABI.

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

Ограничения 0.2.0: до 8 входов `u64`, не больше одного выхода `u64`, не больше
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

На macOS AArch64 (`rustc 1.99.1`, Apple clang, avr-gcc 7.3.0 из Arduino IDE):

- `cargo test --workspace` — 16 тестов, все проходят.
- `cargo clippy --all-targets -- -D warnings` — без предупреждений.
- Пять хост-примеров собираются и запускаются: `Hello from WireASM!`, `55`,
  `55`, `42`, `42`.
- Ассемблер для `aarch64-linux`, `x86_64-linux` и `x86_64-macos` проходит
  `clang -c` с соответствующей тройкой.
- `examples/avr_blink.json` и `examples/avr_uart_hello.json` собираются
  `avr-gcc -mmcu=atmega328p -Os`; `avr-size`: blink — text 218 / bss 132,
  uart — text 224 / data 26 / bss 132.
- `wireasm --build` и `--ports` проверены; реальная загрузка в плату не
  проверялась — под рукой не было Arduino.

Linux и Windows бинарники собираются в GitHub Actions; на этой машине они не
проверялись. macOS-приложение не подписано и не нотаризовано.

## Релизы

Готовые сборки — [WireASM 0.2.0](https://github.com/DOKODS/wireasm/releases/tag/v0.2.0).

`.github/workflows/release.yml` по тегу `v*` собирает архивы:

- `wirestudio-0.2.0-aarch64-macos.tar.gz`
- `wirestudio-0.2.0-x86_64-macos.tar.gz`
- `wirestudio-0.2.0-x86_64-linux.tar.gz`
- `wirestudio-0.2.0-x86_64-windows.zip`

В каждом архиве `wirestudio`, `wireasm`, README, лицензия и changelog.
Локальные архивы кладутся в `releases/`, подробности — в
[releases/README.md](releases/README.md).

## Лицензия

MIT, см. [LICENSE](LICENSE).

---

**English.** WireASM is a visual assembler language: Blueprint-style graphs
compile to real AArch64, x86-64 or ATmega328P assembly. WireStudio is a native
Rust/egui IDE for Linux, Windows and macOS, with MCU nodes, firmware build and
flash (`avr-gcc`/`avrdude`), custom assembler nodes, node libraries, and
English/Russian UI. Build with `cargo build --release`; see the commands above.
