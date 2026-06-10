import 'quill/dist/quill.snow.css';

import Quill from 'quill';
import { useEffect, useMemo, useRef } from 'react';

export type RichTextLeaf = {
  bold?: boolean;
  italic?: boolean;
  text: string;
  underline?: boolean;
};

export type RichTextBlock = {
  children: RichTextLeaf[];
  type: 'blockquote' | 'h1' | 'h2' | 'h3' | 'p';
};

export type RichTextValue = RichTextBlock[];

type QuillRichTextEditorProps = {
  ariaLabel?: string;
  onChange: (value: RichTextValue) => void;
  placeholder?: string;
  value: RichTextValue;
};

type QuillAttributeMap = Record<string, unknown> | undefined;
type QuillOp = {
  attributes?: QuillAttributeMap;
  insert?: string | Record<string, unknown>;
};

const fallbackValue: RichTextValue = [{ type: 'p', children: [{ text: '' }] }];
const supportedFormats = ['bold', 'italic', 'underline', 'header', 'blockquote'];

function normalizedValue(value: RichTextValue): RichTextValue {
  return value.length > 0 ? value : fallbackValue;
}

function inlineAttributes(leaf: RichTextLeaf): Record<string, true> | undefined {
  const attributes: Record<string, true> = {};

  if (leaf.bold) {
    attributes.bold = true;
  }

  if (leaf.italic) {
    attributes.italic = true;
  }

  if (leaf.underline) {
    attributes.underline = true;
  }

  return Object.keys(attributes).length > 0 ? attributes : undefined;
}

function blockAttributes(type: RichTextBlock['type']): Record<string, true | 1 | 2 | 3> | undefined {
  if (type === 'h1') {
    return { header: 1 };
  }

  if (type === 'h2') {
    return { header: 2 };
  }

  if (type === 'h3') {
    return { header: 3 };
  }

  if (type === 'blockquote') {
    return { blockquote: true };
  }

  return undefined;
}

function valueToQuillOps(value: RichTextValue): QuillOp[] {
  return normalizedValue(value).flatMap((block) => {
    const textOps = (block.children.length > 0 ? block.children : [{ text: '' }]).filter((leaf) => leaf.text.length > 0).map((leaf) => ({ insert: leaf.text, attributes: inlineAttributes(leaf) }));
    return [...textOps, { insert: '\n', attributes: blockAttributes(block.type) }];
  });
}

function blockTypeFromAttributes(attributes: QuillAttributeMap): RichTextBlock['type'] {
  if (attributes?.header === 1) {
    return 'h1';
  }

  if (attributes?.header === 2) {
    return 'h2';
  }

  if (attributes?.header === 3) {
    return 'h3';
  }

  if (attributes?.blockquote) {
    return 'blockquote';
  }

  return 'p';
}

function leafFromText(text: string, attributes: QuillAttributeMap): RichTextLeaf {
  return {
    text,
    ...(attributes?.bold ? { bold: true } : {}),
    ...(attributes?.italic ? { italic: true } : {}),
    ...(attributes?.underline ? { underline: true } : {})
  };
}

function quillOpsToValue(ops: QuillOp[]): RichTextValue {
  const blocks: RichTextValue = [];
  let children: RichTextLeaf[] = [];

  ops.forEach((op) => {
    if (typeof op.insert !== 'string') {
      return;
    }

    const segments = op.insert.split('\n');
    segments.forEach((segment, index) => {
      if (segment.length > 0) {
        children.push(leafFromText(segment, op.attributes));
      }

      if (index < segments.length - 1) {
        blocks.push({ type: blockTypeFromAttributes(op.attributes), children: children.length > 0 ? children : [{ text: '' }] });
        children = [];
      }
    });
  });

  if (children.length > 0) {
    blocks.push({ type: 'p', children });
  }

  return blocks.length > 0 ? blocks : fallbackValue;
}

function plainTextToValue(text: string): RichTextValue {
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  return (lines.length > 0 ? lines : ['']).map((line) => ({ type: 'p', children: [{ text: line }] }));
}

export function QuillRichTextEditor({ ariaLabel = '富文本内容', onChange, placeholder = '请输入理财介绍', value }: QuillRichTextEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const quillRef = useRef<Quill | null>(null);
  const onChangeRef = useRef(onChange);
  const initialValueRef = useRef(normalizedValue(value));
  const initialSerializedValueRef = useRef(JSON.stringify(initialValueRef.current));
  const internalValueRef = useRef('');
  const serializedValue = useMemo(() => JSON.stringify(normalizedValue(value)), [value]);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    if (!editorRef.current || !toolbarRef.current || quillRef.current) {
      return;
    }

    const quill = new Quill(editorRef.current, {
      formats: supportedFormats,
      modules: { toolbar: toolbarRef.current },
      placeholder,
      theme: 'snow'
    });
    quillRef.current = quill;
    quill.root.setAttribute('aria-label', ariaLabel);
    quill.root.setAttribute('role', 'textbox');
    quill.root.setAttribute('spellcheck', 'true');
    quill.setContents(valueToQuillOps(initialValueRef.current), 'silent');
    internalValueRef.current = initialSerializedValueRef.current;

    const emitChange = () => {
      const nextValue = quillOpsToValue(quill.getContents().ops as QuillOp[]);
      internalValueRef.current = JSON.stringify(nextValue);
      onChangeRef.current(nextValue);
    };

    const syncSyntheticInput = (event: Event) => {
      if (event.isTrusted) {
        return;
      }

      const nextValue = plainTextToValue(quill.root.innerText);
      quill.setContents(valueToQuillOps(nextValue), 'silent');
      internalValueRef.current = JSON.stringify(nextValue);
      onChangeRef.current(nextValue);
    };

    quill.on('text-change', emitChange);
    quill.root.addEventListener('input', syncSyntheticInput);

    return () => {
      quill.off('text-change', emitChange);
      quill.root.removeEventListener('input', syncSyntheticInput);
      quillRef.current = null;
      if (editorRef.current) {
        editorRef.current.innerHTML = '';
      }
    };
  }, [ariaLabel, placeholder]);

  useEffect(() => {
    const quill = quillRef.current;
    if (!quill || internalValueRef.current === serializedValue) {
      return;
    }

    quill.setContents(valueToQuillOps(normalizedValue(value)), 'silent');
    internalValueRef.current = serializedValue;
  }, [serializedValue, value]);

  return (
    <div className="quill-rich-text-editor" data-quill-editor="true">
      <div className="quill-rich-text-toolbar ql-toolbar" ref={toolbarRef} role="toolbar" aria-label="富文本工具栏">
        <span className="ql-formats">
          <select className="ql-header" defaultValue="" aria-label="块类型">
            <option value="">段落</option>
            <option value="1">H1</option>
            <option value="2">H2</option>
            <option value="3">H3</option>
          </select>
          <button className="ql-blockquote" type="button">
            引用
          </button>
        </span>
        <span className="ql-formats">
          <button className="ql-bold" type="button">
            加粗
          </button>
          <button className="ql-italic" type="button">
            斜体
          </button>
          <button className="ql-underline" type="button">
            下划线
          </button>
        </span>
      </div>
      <div className="quill-rich-text-container" ref={editorRef} />
    </div>
  );
}
