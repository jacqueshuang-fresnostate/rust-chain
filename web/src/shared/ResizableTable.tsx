import { Table } from '@douyinfe/semi-ui';
import type { ColumnProps, ColumnTitle, ColumnTitleProps, RowSelection, TableComponents, TableProps } from '@douyinfe/semi-ui/lib/es/table/interface';
import {
  createContext,
  createElement,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type SyntheticEvent,
  type ElementType,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';

export const RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH = 160;
export const RESIZABLE_TABLE_MIN_COLUMN_WIDTH = 80;
export const RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH = 120;
export const RESIZABLE_TABLE_MAX_COLUMN_WIDTH = 1200;
export const RESIZABLE_TABLE_KEYBOARD_STEP = 16;

const SEMI_UTILITY_COLUMN_WIDTH = 48;
const ACTION_COLUMN_KEY = 'actions';
const ACTION_COLUMN_CLASS_NAME = 'admin-table-action-column';

type AccessibleBodyContextValue = {
  ariaProps: Record<string, unknown>;
  bodyOuter: ElementType;
};

const AccessibleBodyContext = createContext<AccessibleBodyContextValue>({
  ariaProps: {},
  bodyOuter: 'table'
});

const AccessibleBodyOuter = forwardRef<HTMLElement, Record<string, unknown>>(function AccessibleBodyOuter(bodyProps, ref) {
  const { ariaProps, bodyOuter } = useContext(AccessibleBodyContext);
  return createElement(bodyOuter, { ...bodyProps, ...ariaProps, ...(ref ? { ref } : {}) });
});

type ResizableTableProps<RecordType extends Record<string, unknown>> = Omit<TableProps<RecordType>, 'columns' | 'resizable'> & {
  columns: Array<ColumnProps<RecordType>>;
  resizable?: never;
};

type LeafColumnModel<RecordType extends Record<string, unknown>> = {
  accessibleLabel: string;
  column: ColumnProps<RecordType>;
  id: string;
  initialWidth: number;
  minWidth: number;
  safeKey?: string;
};

type ColumnModel<RecordType extends Record<string, unknown>> = {
  children?: Array<ColumnModel<RecordType>>;
  column: ColumnProps<RecordType>;
  leaf?: LeafColumnModel<RecordType>;
};

type PointerResizeState = {
  captureTarget: HTMLSpanElement;
  columnId: string;
  minWidth: number;
  pointerId: number;
  startWidth: number;
  startX: number;
};

function isActionColumn<RecordType extends Record<string, unknown>>(column: ColumnProps<RecordType>) {
  return column.key === ACTION_COLUMN_KEY;
}

function minimumColumnWidth<RecordType extends Record<string, unknown>>(column: ColumnProps<RecordType>) {
  return isActionColumn(column) ? RESIZABLE_TABLE_ACTION_COLUMN_MIN_WIDTH : RESIZABLE_TABLE_MIN_COLUMN_WIDTH;
}

function clampColumnWidth(width: number, minWidth = RESIZABLE_TABLE_MIN_COLUMN_WIDTH) {
  return Math.min(RESIZABLE_TABLE_MAX_COLUMN_WIDTH, Math.max(minWidth, Math.round(width)));
}

function declaredColumnWidth<RecordType extends Record<string, unknown>>(column: ColumnProps<RecordType>, minWidth: number) {
  const width = typeof column.width === 'number' && Number.isFinite(column.width) ? column.width : RESIZABLE_TABLE_DEFAULT_COLUMN_WIDTH;
  return clampColumnWidth(width, minWidth);
}

type BaseColumnIdentity = {
  kind: 'dataIndex' | 'key' | 'path';
  value: string;
};

function baseColumnIdentity<RecordType extends Record<string, unknown>>(column: ColumnProps<RecordType>, path: number[]): BaseColumnIdentity {
  if (column.key !== undefined && column.key !== null) {
    return { kind: 'key', value: String(column.key) };
  }
  if (column.dataIndex) {
    return { kind: 'dataIndex', value: String(column.dataIndex) };
  }
  return { kind: 'path', value: path.join('.') };
}

function baseColumnIdentityKey(identity: BaseColumnIdentity) {
  return JSON.stringify([identity.kind, identity.value]);
}

function columnId(identity: BaseColumnIdentity, path: number[], duplicate: boolean) {
  return JSON.stringify(duplicate ? [identity.kind, identity.value, path.join('.')] : [identity.kind, identity.value]);
}

function accessibleColumnLabel<RecordType extends Record<string, unknown>>(column: ColumnProps<RecordType>, path: number[]) {
  if (typeof column.title === 'string' || typeof column.title === 'number') {
    return String(column.title);
  }
  if (column.dataIndex) {
    return column.dataIndex;
  }
  if (column.key !== undefined && column.key !== null) {
    return String(column.key);
  }
  return `第 ${path[path.length - 1] + 1} 列`;
}

function createColumnModel<RecordType extends Record<string, unknown>>(columns: Array<ColumnProps<RecordType>>) {
  const identityCounts = new Map<string, number>();
  const leaves: Array<LeafColumnModel<RecordType>> = [];

  const countIdentities = (items: Array<ColumnProps<RecordType>>, parentPath: number[]) => {
    items.forEach((column, index) => {
      const path = [...parentPath, index];
      if (Array.isArray(column.children) && column.children.length > 0) {
        countIdentities(column.children, path);
        return;
      }
      const identityKey = baseColumnIdentityKey(baseColumnIdentity(column, path));
      identityCounts.set(identityKey, (identityCounts.get(identityKey) ?? 0) + 1);
    });
  };

  countIdentities(columns, []);

  const visit = (items: Array<ColumnProps<RecordType>>, parentPath: number[]): Array<ColumnModel<RecordType>> =>
    items.map((column, index) => {
      const path = [...parentPath, index];
      if (Array.isArray(column.children) && column.children.length > 0) {
        return {
          children: visit(column.children, path),
          column
        };
      }

      const identity = baseColumnIdentity(column, path);
      const identityKey = baseColumnIdentityKey(identity);
      const duplicateIdentity = (identityCounts.get(identityKey) ?? 0) > 1;
      const id = columnId(identity, path, duplicateIdentity);
      const minWidth = minimumColumnWidth(column);
      const leaf: LeafColumnModel<RecordType> = {
        accessibleLabel: accessibleColumnLabel(column, path),
        column,
        id,
        initialWidth: declaredColumnWidth(column, minWidth),
        minWidth,
        safeKey: duplicateIdentity ? id : undefined
      };
      leaves.push(leaf);
      return { column, leaf };
    });

  return { leaves, tree: visit(columns, []) };
}

function initialWidthMap<RecordType extends Record<string, unknown>>(leaves: Array<LeafColumnModel<RecordType>>) {
  return Object.fromEntries(leaves.map((leaf) => [leaf.id, leaf.initialWidth]));
}

function renderColumnTitle(title: ColumnTitle | undefined, titleProps?: ColumnTitleProps): ReactNode {
  return typeof title === 'function' ? title(titleProps) : title;
}

function utilityColumnsWidth<RecordType extends Record<string, unknown>>(
  rowSelection: RowSelection<RecordType> | undefined,
  expandedRowRender: TableProps<RecordType>['expandedRowRender'],
  hideExpandedColumn: TableProps<RecordType>['hideExpandedColumn']
) {
  let width = 0;
  if (rowSelection && !(typeof rowSelection === 'object' && rowSelection.hidden)) {
    const declaredWidth = typeof rowSelection === 'object' ? rowSelection.width : undefined;
    if (typeof declaredWidth === 'number' && Number.isFinite(declaredWidth) && declaredWidth >= 0) {
      width += declaredWidth;
    } else if (typeof declaredWidth === 'string' && /^\d+(?:\.\d+)?px$/u.test(declaredWidth.trim())) {
      width += Number.parseFloat(declaredWidth);
    } else {
      width += SEMI_UTILITY_COLUMN_WIDTH;
    }
  }
  if (typeof expandedRowRender === 'function' && hideExpandedColumn === false) {
    width += SEMI_UTILITY_COLUMN_WIDTH;
  }
  return width;
}

function tableAriaProps(props: Record<string, unknown>) {
  return Object.fromEntries(Object.entries(props).filter(([key]) => key.startsWith('aria-')));
}

function tableComponentsWithAccessibleBody(components: TableComponents | undefined): TableComponents {
  return {
    ...components,
    body: {
      ...components?.body,
      outer: AccessibleBodyOuter
    }
  };
}

type ColumnResizeHandleProps = {
  columnId: string;
  dragging: boolean;
  label: string;
  minWidth: number;
  onKeyResize: (columnId: string, width: number) => void;
  onPointerResizeStart: (columnId: string, width: number, minWidth: number, event: ReactPointerEvent<HTMLSpanElement>) => void;
  titleContent: ReactNode;
  width: number;
};

function ColumnResizeHandle({ columnId, dragging, label, minWidth, onKeyResize, onPointerResizeStart, titleContent, width }: ColumnResizeHandleProps) {
  const handleKeyDown = (event: ReactKeyboardEvent<HTMLSpanElement>) => {
    let nextWidth: number | null = null;
    if (event.key === 'ArrowLeft') {
      nextWidth = width - RESIZABLE_TABLE_KEYBOARD_STEP;
    } else if (event.key === 'ArrowRight') {
      nextWidth = width + RESIZABLE_TABLE_KEYBOARD_STEP;
    } else if (event.key === 'Home') {
      nextWidth = minWidth;
    } else if (event.key === 'End') {
      nextWidth = RESIZABLE_TABLE_MAX_COLUMN_WIDTH;
    }

    if (nextWidth === null) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    onKeyResize(columnId, nextWidth);
  };

  const stopHeaderAction = (event: SyntheticEvent<HTMLSpanElement>) => {
    event.stopPropagation();
  };

  return (
    <span className="admin-resizable-table-header">
      <span className="admin-resizable-table-header-content">{titleContent}</span>
      <span
        aria-label={`调整${label}列宽`}
        aria-orientation="vertical"
        aria-valuemax={RESIZABLE_TABLE_MAX_COLUMN_WIDTH}
        aria-valuemin={minWidth}
        aria-valuenow={width}
        className={`admin-table-column-resize-handle${dragging ? ' is-dragging' : ''}`}
        data-column-key={columnId}
        onClick={stopHeaderAction}
        onKeyDown={handleKeyDown}
        onPointerDown={(event) => onPointerResizeStart(columnId, width, minWidth, event)}
        role="separator"
        tabIndex={0}
      />
    </span>
  );
}

function areWidthMapsEqual(current: Record<string, number>, next: Record<string, number>) {
  const currentKeys = Object.keys(current);
  const nextKeys = Object.keys(next);
  return currentKeys.length === nextKeys.length && nextKeys.every((key) => current[key] === next[key]);
}

function releasePointerCapture(pointerResize: PointerResizeState | null) {
  if (!pointerResize || typeof pointerResize.captureTarget.releasePointerCapture !== 'function') {
    return;
  }
  try {
    pointerResize.captureTarget.releasePointerCapture(pointerResize.pointerId);
  } catch {
    // The browser may already have released capture after pointercancel.
  }
}

export function ResizableTable<RecordType extends Record<string, unknown>>(props: ResizableTableProps<RecordType>) {
  const {
    className,
    columns,
    components,
    expandedRowRender,
    hideExpandedColumn,
    resizable: ignoredSemiResizable,
    rowSelection,
    scroll,
    ...tableProps
  } = props;
  void ignoredSemiResizable;

  const columnModel = useMemo(() => createColumnModel(columns), [columns]);
  const accessibleBodyContext = useMemo<AccessibleBodyContextValue>(
    () => ({
      ariaProps: tableAriaProps(props as Record<string, unknown>),
      bodyOuter: (components?.body?.outer ?? 'table') as ElementType
    }),
    [components?.body?.outer, props]
  );
  const controlledComponents = useMemo(
    () => tableComponentsWithAccessibleBody(components),
    [
      components?.table,
      components?.header?.outer,
      components?.header?.wrapper,
      components?.header?.row,
      components?.header?.cell,
      components?.body?.outer,
      components?.body?.wrapper,
      components?.body?.row,
      components?.body?.cell,
      components?.body?.colgroup?.wrapper,
      components?.body?.colgroup?.col,
      components?.footer?.outer,
      components?.footer?.wrapper,
      components?.footer?.row,
      components?.footer?.cell
    ]
  );
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>(() => initialWidthMap(columnModel.leaves));
  const [draggingColumnId, setDraggingColumnId] = useState<string | null>(null);
  const pointerResizeRef = useRef<PointerResizeState | null>(null);
  const initialWidthsRef = useRef<Record<string, number>>(initialWidthMap(columnModel.leaves));

  useEffect(() => {
    const previousInitialWidths = initialWidthsRef.current;
    const nextInitialWidths = initialWidthMap(columnModel.leaves);
    initialWidthsRef.current = nextInitialWidths;
    setColumnWidths((current) => {
      const next = Object.fromEntries(
        columnModel.leaves.map((leaf) => [
          leaf.id,
          Object.hasOwn(current, leaf.id) && previousInitialWidths[leaf.id] === leaf.initialWidth
            ? current[leaf.id]
            : leaf.initialWidth
        ])
      );
      return areWidthMapsEqual(current, next) ? current : next;
    });
  }, [columnModel]);

  useEffect(() => {
    if (draggingColumnId && !columnModel.leaves.some((leaf) => leaf.id === draggingColumnId)) {
      releasePointerCapture(pointerResizeRef.current);
      pointerResizeRef.current = null;
      setDraggingColumnId(null);
    }
  }, [columnModel, draggingColumnId]);

  const updateColumnWidth = useCallback((columnId: string, width: number, minWidth = RESIZABLE_TABLE_MIN_COLUMN_WIDTH) => {
    setColumnWidths((current) => {
      const nextWidth = clampColumnWidth(width, minWidth);
      return current[columnId] === nextWidth ? current : { ...current, [columnId]: nextWidth };
    });
  }, []);

  useEffect(() => {
    if (!draggingColumnId) {
      return undefined;
    }

    const preventSelection = (event: Event) => event.preventDefault();
    const handlePointerMove = (event: PointerEvent) => {
      const pointerResize = pointerResizeRef.current;
      if (!pointerResize || event.pointerId !== pointerResize.pointerId) {
        return;
      }
      event.preventDefault();
      updateColumnWidth(pointerResize.columnId, pointerResize.startWidth + event.clientX - pointerResize.startX, pointerResize.minWidth);
    };
    const finishPointerResize = (event: PointerEvent) => {
      const pointerResize = pointerResizeRef.current;
      if (!pointerResize || event.pointerId !== pointerResize.pointerId) {
        return;
      }
      event.preventDefault();
      releasePointerCapture(pointerResize);
      pointerResizeRef.current = null;
      setDraggingColumnId(null);
    };

    document.body.classList.add('admin-table-column-resizing');
    document.addEventListener('pointermove', handlePointerMove, { passive: false });
    document.addEventListener('pointerup', finishPointerResize, { passive: false });
    document.addEventListener('pointercancel', finishPointerResize, { passive: false });
    document.addEventListener('selectstart', preventSelection);

    return () => {
      document.body.classList.remove('admin-table-column-resizing');
      document.removeEventListener('pointermove', handlePointerMove);
      document.removeEventListener('pointerup', finishPointerResize);
      document.removeEventListener('pointercancel', finishPointerResize);
      document.removeEventListener('selectstart', preventSelection);
      releasePointerCapture(pointerResizeRef.current);
      pointerResizeRef.current = null;
    };
  }, [draggingColumnId, updateColumnWidth]);

  const startPointerResize = useCallback(
    (columnId: string, width: number, minWidth: number, event: ReactPointerEvent<HTMLSpanElement>) => {
      if (event.button !== 0 || pointerResizeRef.current) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      window.getSelection()?.removeAllRanges();
      event.currentTarget.focus({ preventScroll: true });
      event.currentTarget.setPointerCapture?.(event.pointerId);
      pointerResizeRef.current = {
        captureTarget: event.currentTarget,
        columnId,
        minWidth,
        pointerId: event.pointerId,
        startWidth: width,
        startX: event.clientX
      };
      setDraggingColumnId(columnId);
    },
    []
  );

  const controlledColumns = useMemo(() => {
    const visit = (models: Array<ColumnModel<RecordType>>): Array<ColumnProps<RecordType>> =>
      models.map((model) => {
        if (model.children) {
          return { ...model.column, children: visit(model.children) };
        }

        const leaf = model.leaf as LeafColumnModel<RecordType>;
        const width = columnWidths[leaf.id] ?? leaf.initialWidth;
        const originalOnHeaderCell = leaf.column.onHeaderCell;
        return {
          ...leaf.column,
          className: isActionColumn(leaf.column)
            ? [leaf.column.className, ACTION_COLUMN_CLASS_NAME].filter(Boolean).join(' ')
            : leaf.column.className,
          key: leaf.safeKey ?? leaf.column.key,
          onHeaderCell: (record, columnIndex, index) => {
            const headerCellProps = originalOnHeaderCell?.(record, columnIndex, index) ?? {};
            return {
              ...headerCellProps,
              'aria-label': headerCellProps['aria-label'] ?? leaf.accessibleLabel
            };
          },
          title: (titleProps?: ColumnTitleProps) => (
            <ColumnResizeHandle
              columnId={leaf.id}
              dragging={draggingColumnId === leaf.id}
              label={leaf.accessibleLabel}
              minWidth={leaf.minWidth}
              onKeyResize={(columnId, nextWidth) => updateColumnWidth(columnId, nextWidth, leaf.minWidth)}
              onPointerResizeStart={startPointerResize}
              titleContent={renderColumnTitle(leaf.column.title, titleProps)}
              width={width}
            />
          ),
          width
        };
      });

    return visit(columnModel.tree);
  }, [columnModel, columnWidths, draggingColumnId, startPointerResize, updateColumnWidth]);

  const horizontalScrollWidth = useMemo(
    () =>
      columnModel.leaves.reduce(
        (total, leaf) => total + (columnWidths[leaf.id] ?? leaf.initialWidth),
        utilityColumnsWidth(rowSelection, expandedRowRender, hideExpandedColumn)
      ),
    [columnModel, columnWidths, expandedRowRender, hideExpandedColumn, rowSelection]
  );

  return (
    <AccessibleBodyContext.Provider value={accessibleBodyContext}>
      <Table<RecordType>
        {...tableProps}
        className={['admin-resizable-table', className].filter(Boolean).join(' ')}
        columns={controlledColumns}
        components={controlledComponents}
        expandedRowRender={expandedRowRender}
        hideExpandedColumn={hideExpandedColumn}
        rowSelection={rowSelection}
        scroll={{ ...scroll, x: horizontalScrollWidth }}
      />
    </AccessibleBodyContext.Provider>
  );
}

export type { ResizableTableProps };
