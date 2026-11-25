/**
 * A read-only window onto one source file (M18 experiment).
 *
 * The lines come from `cartograph-desktop::source::excerpt`, which serves only
 * files the analysis cites. Highlighting is a small lexical colouring for
 * readability — comments, strings, numbers and keywords — and carries no
 * meaning: it is not a parse, and nothing is inferred from it.
 */

import { Fragment, type ReactNode, useEffect, useRef } from "react";

import type { SourceExcerpt } from "./api";

const KEYWORDS = new Set([
  // Python
  "def", "class", "return", "import", "from", "as", "if", "elif", "else", "for", "while",
  "try", "except", "finally", "with", "async", "await", "lambda", "pass", "raise", "yield",
  "None", "True", "False", "self", "in", "is", "not", "and", "or", "global", "nonlocal",
  // TypeScript / JavaScript
  "function", "const", "let", "var", "export", "default", "new", "this", "typeof",
  "interface", "type", "extends", "implements", "null", "undefined", "true", "false",
  "switch", "case", "break", "continue", "throw", "catch", "of",
]);

const TOKEN =
  /(#.*$|\/\/.*$)|("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)|(\b\d+(?:\.\d+)?\b)|([A-Za-z_$][\w$]*)/g;

function colour(line: string, python: boolean): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  for (const match of line.matchAll(TOKEN)) {
    const at = match.index ?? 0;
    if (at > last) {
      out.push(line.slice(last, at));
    }
    const [text, comment, string, number, word] = match;
    if (comment !== undefined && (python ? comment.startsWith("#") : comment.startsWith("//"))) {
      out.push(<span className="bx-tok-comment" key={at}>{text}</span>);
      last = line.length;
      break;
    } else if (string !== undefined) {
      out.push(<span className="bx-tok-string" key={at}>{text}</span>);
    } else if (number !== undefined) {
      out.push(<span className="bx-tok-number" key={at}>{text}</span>);
    } else if (word !== undefined && KEYWORDS.has(word)) {
      out.push(<span className="bx-tok-keyword" key={at}>{text}</span>);
    } else {
      out.push(text);
    }
    last = at + text.length;
  }
  if (last < line.length) {
    out.push(line.slice(last));
  }
  return out;
}

interface Props {
  excerpt: SourceExcerpt;
  onClose: () => void;
}

export default function SourcePreview({ excerpt, onClose }: Props) {
  const python = excerpt.file.endsWith(".py");
  const section = useRef<HTMLElement | null>(null);
  const marked = useRef<HTMLSpanElement | null>(null);
  // Bring the preview, then its marked line, into view when it opens.
  useEffect(() => {
    section.current?.scrollIntoView({ block: "nearest" });
    const line = marked.current;
    const box = line?.closest("pre");
    if (line !== null && line !== undefined && box !== null && box !== undefined) {
      box.scrollTop = line.offsetTop - box.clientHeight / 2;
    }
  }, [excerpt]);
  return (
    <section ref={section} className="bx-source" aria-label={`Source of ${excerpt.file}`}>
      <header className="bx-source-bar">
        <code className="bx-source-file">{excerpt.file}</code>
        <span className="bx-muted">
          lines {excerpt.firstLine}–{excerpt.firstLine + excerpt.lines.length - 1} of{" "}
          {excerpt.totalLines}
        </span>
        <button type="button" className="bx-icon-button" onClick={onClose} aria-label="Close source">
          ×
        </button>
      </header>
      <pre className="bx-source-code" tabIndex={0}>
        {excerpt.lines.map((line, index) => {
          const number = excerpt.firstLine + index;
          const isMarked = number === excerpt.highlight;
          return (
            <Fragment key={number}>
              <span
                ref={isMarked ? marked : undefined}
                className={isMarked ? "bx-line bx-line-marked" : "bx-line"}
                aria-current={isMarked ? "true" : undefined}
              >
                <span className="bx-line-number" aria-hidden="true">
                  {number}
                </span>
                <span className="bx-line-text">{colour(line, python)}</span>
              </span>
              {"\n"}
            </Fragment>
          );
        })}
      </pre>
    </section>
  );
}
