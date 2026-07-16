import 'quill/dist/quill.snow.css';

import { IconUpload } from '@douyinfe/semi-icons';
import { Button, Toast, Upload } from '@douyinfe/semi-ui';
import type { customRequestArgs } from '@douyinfe/semi-ui/lib/es/upload';
import Quill from 'quill';
import { useEffect, useMemo, useRef } from 'react';

import { ApiError } from '../api/client';
import { uploadAdminImageFile } from './AdminImageUpload';

export type RichTextLeaf = {
  bold?: boolean;
  italic?: boolean;
  text: string;
  underline?: boolean;
};

export type RichTextTextBlock = {
  children: RichTextLeaf[];
  type: 'blockquote' | 'h1' | 'h2' | 'h3' | 'p';
};

export type RichTextImageBlock = {
  alt?: string;
  type: 'image';
  url: string;
};

export type RichTextBlock = RichTextTextBlock | RichTextImageBlock;
export type RichTextValue = RichTextBlock[];

type QuillRichTextEditorProps = {
  ariaLabel?: string;
  enableImageUpload?: boolean;
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
const supportedFormats = ['bold', 'italic', 'underline', 'header', 'blockquote', 'image'];

function normalizedValue(value: RichTextValue): RichTextValue {
  return value.length > 0 ? value : fallbackValue;
}

function isImageBlock(block: RichTextBlock): block is RichTextImageBlock {
  return block.type === 'image';
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

function blockAttributes(type: RichTextTextBlock['type']): Record<string, true | 1 | 2 | 3> | undefined {
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
    if (isImageBlock(block)) {
      const url = block.url.trim();
      return url ? [{ insert: { image: url } }, { insert: '\n' }] : [];
    }

    const textOps = (block.children.length > 0 ? block.children : [{ text: '' }]).filter((leaf) => leaf.text.length > 0).map((leaf) => ({ insert: leaf.text, attributes: inlineAttributes(leaf) }));
    return [...textOps, { insert: '\n', attributes: blockAttributes(block.type) }];
  });
}

function blockTypeFromAttributes(attributes: QuillAttributeMap): RichTextTextBlock['type'] {
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

function imageUrlFromInsert(insert: Record<string, unknown>): string {
  return typeof insert.image === 'string' ? insert.image.trim() : '';
}

function quillOpsToValue(ops: QuillOp[]): RichTextValue {
  const blocks: RichTextValue = [];
  let children: RichTextLeaf[] = [];
  let skipNextEmptyNewline = false;

  const pushTextBlock = (type: RichTextTextBlock['type']) => {
    blocks.push({ type, children: children.length > 0 ? children : [{ text: '' }] });
    children = [];
  };

  ops.forEach((op) => {
    if (typeof op.insert !== 'string') {
      if (op.insert && typeof op.insert === 'object') {
        const imageUrl = imageUrlFromInsert(op.insert);
        if (!imageUrl) {
          return;
        }
        if (children.length > 0) {
          pushTextBlock('p');
        }
        blocks.push({ type: 'image', url: imageUrl });
        skipNextEmptyNewline = true;
      }
      return;
    }

    const segments = op.insert.split('\n');
    segments.forEach((segment, index) => {
      if (segment.length > 0) {
        children.push(leafFromText(segment, op.attributes));
        skipNextEmptyNewline = false;
      }

      if (index < segments.length - 1) {
        if (children.length > 0) {
          pushTextBlock(blockTypeFromAttributes(op.attributes));
        } else if (skipNextEmptyNewline) {
          skipNextEmptyNewline = false;
        } else {
          pushTextBlock(blockTypeFromAttributes(op.attributes));
        }
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

function isEmptyTextValue(value: RichTextValue): boolean {
  return value.length === 1 && value[0].type !== 'image' && value[0].children.every((leaf) => !leaf.text.trim());
}

function uploadErrorMessage(error: unknown) {
  return error instanceof ApiError || error instanceof Error ? error.message : '图片上传失败';
}

export function QuillRichTextEditor({ ariaLabel = '富文本内容', enableImageUpload = false, onChange, placeholder = '请输入理财介绍', value }: QuillRichTextEditorProps) {
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

  const insertImage = (url: string) => {
    const quill = quillRef.current;
    const imageUrl = url.trim();
    if (!quill || !imageUrl) {
      return;
    }

    const currentValue = quillOpsToValue(quill.getContents().ops as QuillOp[]);
    const nextValue: RichTextValue = [...(isEmptyTextValue(currentValue) ? [] : currentValue), { type: 'image', url: imageUrl }];
    const serializedNextValue = JSON.stringify(nextValue);
    quill.setContents(valueToQuillOps(nextValue), 'silent');
    quill.setSelection(quill.getLength(), 0, 'silent');
    internalValueRef.current = serializedNextValue;
    onChangeRef.current(nextValue);
  };

  const customImageRequest = async (request: customRequestArgs) => {
    try {
      request.onProgress({ loaded: 1, total: 2 });
      const response = await uploadAdminImageFile(request.fileInstance);
      insertImage(response.download_url);
      request.onSuccess(response);
    } catch (error) {
      Toast.error(uploadErrorMessage(error));
      request.onError({ status: error instanceof ApiError ? error.status : 500 });
    }
  };

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
        {enableImageUpload ? (
          <span className="ql-formats quill-rich-text-upload">
            <Upload
              accept=".png,.jpg,.jpeg,.webp,.gif,image/png,image/jpeg,image/webp,image/gif"
              action="/admin/api/v1/uploads/images"
              customRequest={customImageRequest}
              limit={1}
              onSizeError={() => Toast.error('图片大小超过上传配置限制')}
              showUploadList={false}
            >
              <Button icon={<IconUpload aria-hidden="true" />} size="small" theme="borderless" type="tertiary">
                插入图片
              </Button>
            </Upload>
          </span>
        ) : null}
      </div>
      <div className="quill-rich-text-container" ref={editorRef} />
    </div>
  );
}
