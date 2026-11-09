# WireASM 0.1.0 + WireStudio

Визуальный язык поверх ассемблера: граф из узлов, как в Blueprint у Unreal,
компилируется в настоящий `.s` для AArch64 или x86-64.

```
граф JSON ──► wireasm-core ──► .s ──► clang ──► исполняемый файл
                  │
                  └── проверки, диагностика, статистика
```

**WireStudio** — нативная среда на Rust + egui. Никакого браузера, сервера и
телеметрии: один исполняемый файл для Linux, Windows и macOS.

## Что умеет

- Четыре цели: `aarch64-macos`, `aarch64-linux`, `x86_64-linux`, `x86_64-macos`.
- 17 встроенных узлов: поток, переменные, арифметика, сравнения, вывод,
  вызовы, сисколлы, физические регистры и сырой ассемблер.
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
cd WIREASM_AARCH64_X86_0.1.0

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
```

CLI:

```
wireasm <graph.json> [options]
  -o <file.s>        куда писать ассемблер (по умолчанию stdout)
  --target <t>       переопределить цель из графа
  --check            только валидация
  --run              собрать через clang и запустить (лимит 5 с)
```

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

Явная компиляция сохранённого графа кладёт рядом файл `<имя>.s`. Автокомпиляция
после правок обновляет панели, но не пишет файл. Экспорт — File → Export
assembler…, он же сохраняет папку экспорта в настройках.

Настройки (язык, последняя папка, папка экспорта) лежат в:

- macOS: `~/Library/Application Support/WireStudio/settings.json`
- Linux: `~/.config/wirestudio/settings.json` (или `$XDG_CONFIG_HOME`)
- Windows: `%APPDATA%\WireStudio\settings.json`

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

Ограничения 0.1.0: до 8 входов `u64`, не больше одного выхода `u64`, не больше
одного `exec`-входа и одного `exec`-выхода; `exec`-выход требует `exec`-входа.

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

На macOS AArch64 (`rustc 1.99.1`, Apple clang):

- `cargo test --workspace` — 12 тестов, все проходят.
- `cargo clippy --workspace --release` — без предупреждений.
- Все пять примеров собираются и запускаются: `Hello from WireASM!`, `55`,
  `55`, `42`, `42`.
- Ассемблер для `aarch64-linux`, `x86_64-linux` и `x86_64-macos` проходит
  `clang -c` с соответствующей тройкой.

Linux и Windows бинарники собираются в GitHub Actions; на этой машине они не
проверялись. macOS-приложение не подписано и не нотаризовано.

## Релизы

Готовые сборки — [WireASM 0.1.0](https://github.com/DOKODS/wireasm/releases/tag/v0.1.0).

`.github/workflows/release.yml` по тегу `v*` собирает архивы:

- `wirestudio-0.1.0-aarch64-macos.tar.gz`
- `wirestudio-0.1.0-x86_64-macos.tar.gz`
- `wirestudio-0.1.0-x86_64-linux.tar.gz`
- `wirestudio-0.1.0-x86_64-windows.zip`

В каждом архиве `wirestudio`, `wireasm`, README, лицензия и changelog.
Локальные архивы кладутся в `releases/`, подробности — в
[releases/README.md](releases/README.md).

## Лицензия

MIT, см. [LICENSE](LICENSE).

---

**English.** WireASM is a visual assembler language: Blueprint-style graphs
compile to real AArch64 or x86-64 assembly. WireStudio is a native Rust/egui
IDE for Linux, Windows and macOS, with custom assembler nodes, node libraries,
and English/Russian UI. Build with `cargo build --release`; see the commands
above.
