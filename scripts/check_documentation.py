#!/usr/bin/env python3
"""文書のローカルリンクと、掲載したリポジトリ内コマンド入口を静的に検査します。

ネットワーク疎通やコマンドの実行成功はこの検査の対象ではありません。
--self-test は一時的な文書の正常例・否定例で検査器を確認します。

インライン・参照式・画像・HTMLリンクを取り出し、コード例を除外します。
Markdown全体をHTMLへ変換する実装ではなく、このリポジトリの文書検査用です。
見出しのリンクはGitHubの表記に合わせ、明示HTMLアンカーも検査します。
外部URIはローカルファイルと混同せず集計だけを行います。
生成後に存在するdistの道具と、歴史的文書の旧コマンドは実在検査から除外します。
構文の参照: https://spec.commonmark.org/0.31.2/#links
見出しの参照: https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links
"""
from __future__ import annotations

import argparse
import html
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shlex
import string
import tempfile
import tomllib
import unicodedata
import unittest
from urllib.parse import unquote, urlsplit



EXCLUDED = {".git", "node_modules", "dist", "target", ".lake", ".venv", "__pycache__"}
HISTORICAL = {
    "docs/verification/results.md", "docs/verification/portability.md",
    "docs/verification/test-mapping.md", "docs/verification/completion-scope.md",
    "docs/verification/completion-acceptance.md",
}
SHELL_LANGUAGES = {"sh", "bash", "shell", "console", "zsh", "powershell", "pwsh"}
MAX_DOCUMENT_BYTES = 4 * 1024 * 1024


def blank(text: str) -> str:
    return "".join("\n" if character == "\n" else " " for character in text)


def escaped(text: str, index: int) -> bool:
    count = 0
    while index > 0 and text[index - 1] == "\\":
        index -= 1
        count += 1
    return count % 2 == 1


def block_content(text: str) -> tuple[str, list[tuple[int, str, str]]]:
    """Keep positions stable while masking comments and fenced/indented code."""
    text = re.sub(r"<!--.*?-->", lambda match: blank(match[0]), text, flags=re.S)
    text = re.sub(r"<(pre|code)\b[^>]*>.*?</\1\s*>",
                  lambda match: blank(match[0]), text, flags=re.S | re.I)
    body, blocks, code = [], [], []
    fence, language, first_line = "", "", 0
    previous_blank, indented, in_list = True, False, False
    for number, line in enumerate(text.splitlines(keepends=True), 1):
        unquoted = re.sub(r"^[ ]{0,3}(?:>[ ]?)*", "", line)
        if fence:
            body.append(blank(line))
            if re.match(r"^" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}[ \t]*\r?\n?$", unquoted):
                blocks.append((first_line, language, "".join(code)))
                fence, code = "", []
            else:
                code.append(unquoted)
            continue
        opening = re.match(r"^(\x60{3,}|~{3,})([^\n]*)", unquoted)
        if opening:
            fence = opening[1]
            language = opening[2].strip().split(maxsplit=1)[0].lower() if opening[2].strip() else ""
            first_line = number + 1
            body.append(blank(line))
            continue
        list_item = re.match(r"^[ ]{0,3}(?:[-+*]|\d+[.)])[ \t]+", line)
        if list_item:
            in_list = True
        elif line.strip() and not line.startswith((" ", "\t")):
            in_list = False
        is_indented = line.startswith(("    ", "\t")) and not in_list and (previous_blank or indented)
        if is_indented or (indented and not line.strip()):
            body.append(blank(line))
            indented = True
        else:
            body.append(line)
            indented = False
        previous_blank = not line.strip()
    if fence:
        blocks.append((first_line, language, "".join(code)))
    return "".join(body), blocks


def inline_content(text: str) -> tuple[str, list[tuple[int, str]]]:
    result, spans, cursor = list(text), [], 0
    while cursor < len(text):
        if text[cursor] != chr(96) or escaped(text, cursor):
            cursor += 1
            continue
        end = cursor
        while end < len(text) and text[end] == chr(96):
            end += 1
        delimiter = text[cursor:end]
        closing = re.search(r"(?<!\x60)" + re.escape(delimiter) + r"(?!\x60)", text[end:])
        if closing is None:
            cursor = end
            continue
        finish = end + closing.start() + len(delimiter)
        spans.append((text.count("\n", 0, cursor) + 1, text[end:finish - len(delimiter)]))
        result[cursor:finish] = blank(text[cursor:finish])
        cursor = finish
    return "".join(result), spans


def unescape(value: str) -> str:
    punctuation = re.escape(string.punctuation)
    return html.unescape(re.sub(r"\\([" + punctuation + "])", r"\1", value))


def brackets(text: str) -> dict[int, int]:
    stack, pairs = [], {}
    for index, character in enumerate(text):
        if escaped(text, index):
            continue
        if character == "[":
            stack.append(index)
        elif character == "]" and stack:
            pairs[stack.pop()] = index
    return pairs


def destination(text: str, cursor: int) -> tuple[str, int] | None:
    while cursor < len(text) and text[cursor].isspace():
        cursor += 1
    start = cursor
    if cursor < len(text) and text[cursor] == "<":
        cursor += 1
        while cursor < len(text):
            if text[cursor] == "\n":
                return None
            if text[cursor] == ">" and not escaped(text, cursor):
                return unescape(text[start + 1:cursor]), cursor + 1
            cursor += 1
        return None
    depth = 0
    while cursor < len(text):
        character = text[cursor]
        if character == "\\" and cursor + 1 < len(text):
            cursor += 2
            continue
        if character.isspace() or (character == ")" and depth == 0):
            break
        if character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
        cursor += 1
    return None if depth else (unescape(text[start:cursor]), cursor)


def inline_destination(text: str, cursor: int) -> tuple[str, int] | None:
    found = destination(text, cursor)
    if found is None:
        return None
    target, end = found
    while end < len(text) and text[end].isspace():
        end += 1
    if end < len(text) and text[end] in "\"'(":
        delimiter = ")" if text[end] == "(" else text[end]
        end += 1
        while end < len(text) and (text[end] != delimiter or escaped(text, end)):
            end += 1
        if end == len(text):
            return None
        end += 1
        while end < len(text) and text[end].isspace():
            end += 1
    if end < len(text) and text[end] == ")":
        return target, end + 1
    return None


class HtmlLinks(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.links: list[tuple[int, str, bool]] = []
        self.anchors: set[str] = set()

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if attributes.get("id"):
            self.anchors.add(attributes["id"])
        if tag == "a" and attributes.get("name"):
            self.anchors.add(attributes["name"])
        key = "href" if tag in {"a", "link"} else "src" if tag in {"img", "source"} else None
        if key and attributes.get(key) is not None:
            self.links.append((self.getpos()[0], attributes[key], tag != "a"))

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)


def reference_label(text: str) -> str:
    return " ".join(text.split()).casefold()


def markdown_links(body: str) -> tuple[list[tuple[int, str, bool]], list[tuple[int, str]]]:
    text, _ = inline_content(body)
    pairs = brackets(text)
    references, links, problems, definition_spans = {}, [], [], []
    for match in re.finditer(r"(?m)^[ ]{0,3}(?:>[ ]?)*\[", text):
        start = match.end() - 1
        end = pairs.get(start)
        if end is None or text[start + 1:start + 2] == "^" or text[end + 1:end + 2] != ":":
            continue
        found = destination(text, end + 2)
        if found is None:
            continue
        target, finish = found
        label = reference_label(text[start + 1:end])
        references.setdefault(label, target)
        line = text.count("\n", 0, start) + 1
        links.append((line, target, False))
        newline = text.find("\n", finish)
        definition_spans.append((match.start(), len(text) if newline < 0 else newline))
    excluded_spans = definition_spans[:]
    reference_positions = set()
    for start, end in sorted(pairs.items()):
        if start in reference_positions or any(a <= start < b for a, b in excluded_spans):
            continue
        next_character = text[end + 1:end + 2]
        line = text.count("\n", 0, start) + 1
        image = start > 0 and text[start - 1] == "!" and not escaped(text, start - 1)
        if next_character == "(":
            found = inline_destination(text, end + 2)
            if found is not None:
                links.append((line, found[0], image))
                excluded_spans.append((end + 1, found[1]))
        elif next_character == "[" and end + 1 in pairs:
            label_end = pairs[end + 1]
            reference_positions.add(end + 1)
            label = text[end + 2:label_end] or text[start + 1:end]
            normalized = reference_label(label)
            if normalized in references:
                links.append((line, references[normalized], image))
            else:
                problems.append((line, f"未定義の参照リンク: {label}"))
        else:
            label = reference_label(text[start + 1:end])
            if label in references:
                links.append((line, references[label], image))
    # HTML attributes and autolinks are outside fenced/inline code as well.
    parser = HtmlLinks()
    parser.feed(text)
    links.extend(parser.links)
    for match in re.finditer(r"<([A-Za-z][A-Za-z0-9+.-]*:[^<>\s]+|[^<>\s@]+@[^<>\s@]+)>", text):
        target = match[1]
        if "@" in target and not re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", target):
            target = "mailto:" + target
        links.append((text.count("\n", 0, match.start()) + 1, target, False))
    return links, problems


def heading_slug(text: str) -> str:
    # GitHub-style visible heading text; manual HTML anchors are handled separately.
    text = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"<[^>]*>", "", text)
    text = re.sub(r"(?<!\w)(\*{1,2}|_{1,2})(.+?)\1(?!\w)", r"\2", text)
    text = unescape(text).replace(chr(96), "").lower()
    return "".join(
        "-" if character == " " else character
        for character in text
        if character in " -_" or unicodedata.category(character)[0] in "LNM"
    )


def markdown_anchors(body: str) -> set[str]:
    parser = HtmlLinks()
    parser.feed(body)
    anchors, used = set(parser.anchors), set()
    previous = ""
    for line in body.splitlines():
        atx = re.match(r"^[ ]{0,3}#{1,6}(?:[ \t]+|$)(.*)", line)
        heading = None
        if atx:
            heading = re.sub(r"[ \t]+#+[ \t]*$", "", atx[1]).strip()
        elif previous.strip() and re.match(r"^[ ]{0,3}(?:=+|-+)[ \t]*$", line):
            heading = previous.strip()
        if heading is not None:
            base = heading_slug(heading)
            anchor, suffix = base, 0
            while anchor in used:
                suffix += 1
                anchor = f"{base}-{suffix}"
            used.add(anchor)
            anchors.add(anchor)
        previous = line
    return anchors


def inside(root: Path, target: Path) -> bool:
    return target.resolve().is_relative_to(root.resolve())


def anchor_problem(root: Path, path: Path, fragment: str, image: bool) -> str | None:
    if image or not fragment:
        return None
    if path.is_dir():
        readme = next((path / name for name in ("README.md", "readme.md", "index.md", "index.html")
                       if (path / name).is_file()), None)
        if readme is None:
            return "ディレクトリのアンカーを検査できるREADMEがありません"
        path = readme
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeError, OSError):
        return "テキスト以外のファイルのアンカーは検査できません"
    if path.suffix.lower() in {".md", ".markdown"}:
        body, _ = block_content(text)
        return None if fragment in markdown_anchors(body) else f"見出し・アンカーがありません: #{fragment}"
    if path.suffix.lower() in {".html", ".htm", ".svg"}:
        parser = HtmlLinks()
        parser.feed(text)
        return None if fragment in parser.anchors else f"HTML/SVGアンカーがありません: #{fragment}"
    match = re.fullmatch(r"L([1-9]\d*)(?:-L([1-9]\d*))?", fragment)
    if match:
        first, last = int(match[1]), int(match[2] or match[1])
        return None if first <= last <= len(text.splitlines()) else f"行番号が範囲外です: #{fragment}"
    return f"この種類のファイルで扱えないアンカーです: #{fragment}"


def historical(path: str) -> bool:
    return path.startswith("docs/workdocs/") or path in HISTORICAL


def option(arguments: list[str], *names: str) -> str | None:
    for index, argument in enumerate(arguments):
        for name in names:
            if argument == name:
                return arguments[index + 1] if index + 1 < len(arguments) else None
            if argument.startswith(name + "="):
                return argument[len(name) + 1:]
    return None


def cargo_packages(root: Path) -> tuple[dict[str, tuple[Path, dict]], list[str]]:
    manifest = root / "Cargo.toml"
    if not manifest.is_file():
        return {}, []
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    directories = [root]
    for pattern in data.get("workspace", {}).get("members", []):
        directories.extend(path for path in root.glob(pattern) if path.is_dir())
    packages = {}
    by_directory = {}
    for directory in directories:
        path = directory / "Cargo.toml"
        if not path.is_file():
            continue
        current = tomllib.loads(path.read_text(encoding="utf-8"))
        if "package" in current:
            name = current["package"]["name"]
            packages[name] = (directory, current)
            by_directory[directory.resolve()] = name
    defaults = []
    for pattern in data.get("workspace", {}).get("default-members", []):
        defaults.extend(by_directory[path.resolve()] for path in root.glob(pattern)
                        if path.resolve() in by_directory)
    if not defaults and "package" in data:
        defaults = [data["package"]["name"]]
    return packages, defaults


def cargo_binaries(directory: Path, data: dict) -> dict[str, Path]:
    binaries = {}
    if data.get("package", {}).get("autobins", True):
        if (directory / "src/main.rs").is_file():
            binaries[data["package"]["name"]] = directory / "src/main.rs"
        for path in (directory / "src/bin").glob("*.rs"):
            binaries[path.stem] = path
        for path in (directory / "src/bin").glob("*/main.rs"):
            binaries[path.parent.name] = path
    for binary in data.get("bin", []):
        name = binary["name"]
        target = directory / binary.get("path", f"src/bin/{name}.rs")
        # Explicit bin entries rename an automatically found source; Cargo does
        # not build the same main.rs again under the package's implicit name.
        binaries = {key: path for key, path in binaries.items() if path != target}
        binaries[name] = target
    return binaries


def command_problem(root: Path, cwd: Path, words: list[str], packages: dict, defaults: list[str]) -> tuple[bool, str | None]:
    if not words:
        return False, None
    while words and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", words[0]):
        words = words[1:]
    if not words:
        return False, None
    executable, arguments = words[0], words[1:]
    if executable in {"python", "python3", "uv", "node", "bash", "sh", "source", "."}:
        script = next((arg for arg in arguments if not arg.startswith("-") and
                       (arg.endswith((".py", ".sh", ".ps1", ".mjs", ".js")))), None)
        if script is None:
            return False, None
        target = cwd / script
        # Activation files are generated by bootstrap; they are not source inputs.
        if inside(root, target) and target.resolve().relative_to(root).as_posix().startswith("dist/"):
            return False, None
        return True, None if inside(root, target) and target.is_file() else f"コマンドのファイルがありません: {script}"
    if executable == "cargo":
        if arguments and arguments[0].startswith("+"):
            arguments = arguments[1:]
        if not arguments:
            return False, None
        if "--" in arguments:
            arguments = arguments[:arguments.index("--")]
        selected = option(arguments, "-p", "--package")
        names = [selected] if selected else list(packages) if "--workspace" in arguments else defaults
        if not names or any(name not in packages for name in names):
            return True, f"Cargoパッケージを解決できません: {selected or '(既定)'}"
        binary = option(arguments, "--bin")
        test = option(arguments, "--test")
        if binary:
            if not any(binary in cargo_binaries(*packages[name]) and
                       cargo_binaries(*packages[name])[binary].is_file() for name in names):
                return True, f"Cargoの実行対象がありません: --bin {binary}"
        elif arguments[0] == "run":
            targets = []
            for name in names:
                directory, data = packages[name]
                bins = cargo_binaries(directory, data)
                default = data.get("package", {}).get("default-run")
                targets.extend([default] if default in bins else list(bins))
            if len(targets) != 1:
                return True, "cargo runの対象が一意ではありません。-pと--binを明示してください"
        if test and not any((packages[name][0] / "tests" / (test + ".rs")).is_file() or
                            (packages[name][0] / "tests" / test / "main.rs").is_file() for name in names):
            return True, f"Cargoの結合テストがありません: --test {test}"
        if "--lib" in arguments and not any(
                (packages[name][0] / packages[name][1].get("lib", {}).get("path", "src/lib.rs")).is_file()
                for name in names):
            return True, "選択したCargoパッケージにlib対象がありません"
        return True, None
    if executable == "npm":
        directory = cwd / (option(arguments, "--prefix") or ".")
        manifest = directory / "package.json"
        if not inside(root, manifest) or not manifest.is_file():
            return True, "npmのpackage.jsonがありません"
        data = json.loads(manifest.read_text(encoding="utf-8"))
        script = None
        for index, word in enumerate(arguments):
            if word in {"run", "run-script"}:
                script = arguments[index + 1] if index + 1 < len(arguments) else ""
                break
        if script is None:
            script = next((word for word in arguments if word in {"test", "start", "stop", "restart"}), None)
        if script is not None and script not in data.get("scripts", {}):
            return True, f"npmスクリプトがありません: {script}"
        if "ci" in arguments and not (directory / "package-lock.json").is_file():
            return True, "npm ciが必要とするpackage-lock.jsonがありません"
        return True, None
    if executable == "lake":
        if not any((cwd / name).is_file() for name in ("lakefile.lean", "lakefile.toml")):
            return True, "lakeコマンドの作業ディレクトリにlakefileがありません"
        lean_file = next((arg for arg in arguments if arg.endswith(".lean")), None)
        if lean_file and not (cwd / lean_file).is_file():
            return True, f"Leanの入力ファイルがありません: {lean_file}"
        return True, None
    return False, None


def command_lines(root: Path, body: str, blocks: list) -> list[tuple[int, Path, list[str]]]:
    result = []
    sources = [(line, text) for line, language, text in blocks if language in SHELL_LANGUAGES]
    _, inline = inline_content(body)
    for line, command in inline:
        # Inline prose often names only a tool. Check concrete repository commands.
        if re.match(r"^(?:python3?|uv)\s+.*scripts/\S+\.py(?:\s|$)", command) or (
            re.match(r"^(?:cargo|npm)\s", command) and re.search(r"(?:--prefix|-p |--package|--bin|--workspace)", command)
        ):
            sources.append((line, command))
    for first_line, source in sources:
        cwd, stack = root, []
        source = source.replace("\\\n", " ")
        for offset, line in enumerate(source.splitlines()):
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            if line.lstrip().startswith("$ "):
                line = line.lstrip()[2:]
            try:
                lexer = shlex.shlex(line, posix=True, punctuation_chars=";&|()")
                lexer.whitespace_split = True
                lexer.commenters = "#"
                tokens = list(lexer)
            except ValueError:
                continue  # Non-shell prose inside examples is not executed.
            segment = []
            for token in tokens + [";"]:
                if token not in {";", "&&", "||", "|", "(", ")"}:
                    segment.append(token)
                    continue
                if segment:
                    if segment[0] == "cd" and len(segment) == 2:
                        candidate = (cwd / segment[1]).resolve()
                        if inside(root, candidate):
                            cwd = candidate
                    else:
                        result.append((first_line + offset, cwd, segment))
                    segment = []
                if token == "(":
                    stack.append(cwd)
                elif token == ")" and stack:
                    cwd = stack.pop()
    return result


def check_repository(root: Path) -> dict:
    root = root.resolve()
    report = {
        "success": False, "documents": 0, "local_links": 0, "external_links": 0,
        "repository_commands": 0, "historical_command_documents": 0, "errors": [],
    }
    documents = []
    for directory, names, files in os.walk(root, followlinks=False):
        names[:] = sorted(name for name in names if name not in EXCLUDED)
        for name in sorted(files):
            if Path(name).suffix.lower() in {".md", ".markdown"}:
                documents.append(Path(directory) / name)
    try:
        packages, defaults = cargo_packages(root)
    except (OSError, ValueError) as error:
        report["errors"].append(f"Cargo定義を読めません: {error}")
        packages, defaults = {}, []
    for path in documents:
        relative = path.relative_to(root).as_posix()
        report["documents"] += 1
        if not inside(root, path):
            report["errors"].append(f"{relative}: 文書がリポジトリ外へリンクしています")
            continue
        try:
            if path.stat().st_size > MAX_DOCUMENT_BYTES:
                raise ValueError("Markdownが4MiBを超えています")
            text = path.read_text(encoding="utf-8")
            body, blocks = block_content(text)
            links, problems = markdown_links(body)
        except (OSError, UnicodeError, ValueError) as error:
            report["errors"].append(f"{relative}: 文書を解析できません: {error}")
            continue
        for line, problem in problems:
            report["errors"].append(f"{relative}:{line}: {problem}")
        for line, target, image in links:
            try:
                uri = urlsplit(target)
            except ValueError:
                report["errors"].append(f"{relative}:{line}: URIを解釈できません: {target}")
                continue
            if uri.scheme or uri.netloc:
                report["external_links"] += 1
                continue
            report["local_links"] += 1
            decoded = unquote(uri.path)
            destination_path = (root / decoded.lstrip("/")) if decoded.startswith("/") else (path.parent / decoded) if decoded else path
            problem = None
            if not inside(root, destination_path):
                problem = "リンク先がリポジトリの外に出ます"
            elif not destination_path.exists():
                problem = "リンク先がありません"
            elif image and not destination_path.is_file():
                problem = "画像のリンク先がファイルではありません"
            else:
                problem = anchor_problem(root, destination_path, unquote(uri.fragment), image)
            if problem:
                report["errors"].append(f"{relative}:{line}: {problem}: {target}")
        if historical(relative):
            report["historical_command_documents"] += 1
            continue
        for line, cwd, words in command_lines(root, body, blocks):
            try:
                checked, problem = command_problem(root, cwd, words, packages, defaults)
            except (OSError, ValueError) as error:
                checked, problem = True, f"コマンドの定義を解析できません: {error}"
            report["repository_commands"] += int(checked)
            if problem:
                report["errors"].append(f"{relative}:{line}: {problem}")
    if not documents:
        report["errors"].append("検査するMarkdown文書がありません")
    report["success"] = not report["errors"]
    return report


class DocumentationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzume-documentation-")
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def put(self, name: str, text: str):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def check(self) -> dict:
        return check_repository(self.root)

    def test_inline_reference_images_protocols_and_html_are_supported(self):
        self.put("guide.md", "# 利用方法\n# 利用方法\n<a id=\"manual\"></a>\n")
        self.put("image (one).svg", "<svg><g id=\"one\"/></svg>")
        self.put("a b.md", "# With space\n")
        self.put("README.md", """# Start
[guide](guide.md#利用方法) [twice][REF] ![image](image%20%28one%29.svg)
[REF]: guide.md#%E5%88%A9%E7%94%A8%E6%96%B9%E6%B3%95-1 "title"
[ref][] [ref] [space](<a b.md#with-space>)
<a href="guide.md#manual">HTML</a><img src="image%20%28one%29.svg">
[web](https://example.invalid/path) [email](mailto:test@example.invalid)
[network](//example.invalid/path) <https://example.invalid/> <test@example.invalid>
""")
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertGreaterEqual(result["local_links"], 8)
        self.assertGreaterEqual(result["external_links"], 4)

    def test_missing_path_anchor_image_and_reference_fail(self):
        self.put("guide.md", "# Exists\n")
        self.put("README.md", "[a](missing.md) [b](guide.md#absent) ![c](gone.png) [d][undefined]\n")
        result = self.check()
        self.assertFalse(result["success"])
        self.assertEqual(len(result["errors"]), 4)

    def test_code_examples_are_not_interpreted_as_links(self):
        self.put("README.md", """# Start
\x60[inline](missing-one)\x60
\x60\x60\x60\x60markdown
[block](missing-two)
\x60\x60\x60
\x60\x60\x60\x60
~~~markdown
![image](missing-three)
~~~

    [indented](missing-four)

<!-- [comment](missing-five) -->
""")
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertEqual(result["local_links"], 0)

    def test_nested_labels_and_parentheses_do_not_truncate_targets(self):
        self.put("a(b).md", "# Header\n")
        self.put("photo.svg", "<svg/>")
        self.put("README.md", '[a [nested]](a(b).md "a title") [![picture](photo.svg)](a\\(b\\).md#header)\n')
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertEqual(result["local_links"], 3)

    def test_duplicate_headings_custom_anchors_and_line_anchors(self):
        self.put("guide.md", "# はじめに \x60API\x60\n# はじめに API\n<a name=\"special\"></a>\nSetext\n------\n")
        self.put("code.rs", "one\ntwo\nthree\n")
        self.put("page.html", '<h1 id="heading">Title</h1>')
        self.put("README.md", "[a](guide.md#はじめに-api) [b](guide.md#はじめに-api-1) [c](guide.md#special) [d](guide.md#setext) [e](code.rs#L1-L3) [f](page.html#heading)\n")
        self.assertTrue(self.check()["success"])
        self.put("README.md", "[a](code.rs#L4) [b](page.html#absent)\n")
        self.assertEqual(len(self.check()["errors"]), 2)

    def test_excluded_directories_and_root_relative_links(self):
        self.put("README.md", "[root](/docs/guide.md#guide)\n")
        self.put("docs/guide.md", "# Guide\n")
        for directory in ("node_modules", "dist", ".git", "target"):
            self.put(f"{directory}/README.md", "[missing](not-here)\n")
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertEqual(result["documents"], 2)

    def test_escaping_repository_and_empty_document_sets_are_rejected(self):
        self.assertFalse(self.check()["success"])
        self.put("README.md", "[escape](../outside.md)\n")
        self.assertFalse(self.check()["success"])

    def test_python_cargo_and_npm_entries_are_checked_without_execution(self):
        self.put("Cargo.toml", '[package]\nname="demo"\nversion="0.1.0"\nedition="2021"\n')
        self.put("src/bin/tool.rs", "fn main() {}\n")
        self.put("src/lib.rs", "// core\n")
        self.put("scripts/check.py", "raise RuntimeError('must never execute')\n")
        self.put("frontend/package.json", '{"scripts":{"build":"deliberately-not-an-executable"}}')
        self.put("README.md", """# Commands
\x60\x60\x60sh
python3 scripts/check.py
cargo run -p demo --bin tool -- --help
cargo test -p demo --lib
npm run build --prefix frontend
\x60\x60\x60
""")
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertEqual(result["repository_commands"], 4)
        self.put("README.md", """\x60\x60\x60sh
python3 scripts/missing.py
cargo run -p absent --bin tool
cargo run -p demo --bin absent
npm run missing --prefix frontend
\x60\x60\x60
""")
        result = self.check()
        self.assertFalse(result["success"])
        self.assertEqual(len(result["errors"]), 4)

    def test_historical_commands_are_preserved_but_links_are_checked(self):
        self.put("README.md", "# Current\n")
        self.put("docs/workdocs/old.md", "\x60\x60\x60sh\npython3 removed-script.py\n\x60\x60\x60\n[link](missing.md)\n")
        result = self.check()
        self.assertFalse(result["success"])
        self.assertEqual(len(result["errors"]), 1)
        self.assertEqual(result["historical_command_documents"], 1)

    def test_explicit_binary_name_replaces_implicit_main(self):
        self.put("Cargo.toml", '[package]\nname="demo-cli"\nversion="0.1.0"\n[[bin]]\nname="demo"\npath="src/main.rs"\n')
        self.put("src/main.rs", "fn main() {}\n")
        self.put("README.md", "\x60\x60\x60sh\ncargo run -p demo-cli -- --help\n\x60\x60\x60\n")
        result = self.check()
        self.assertTrue(result["success"], result["errors"])
        self.assertEqual(result["repository_commands"], 1)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DocumentationTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    result = check_repository(args.root.resolve())
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"文書 {result['documents']}件、相対リンク {result['local_links']}件、外部URI {result['external_links']}件、コマンド入口 {result['repository_commands']}件")
        for error in result["errors"]:
            print(error)
        print("文書検査: " + ("成功" if result["success"] else "失敗"))
    return 0 if result["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
