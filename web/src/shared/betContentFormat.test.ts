import { describe, expect, it } from 'vitest';

import { formatAdminBetContent, isAdminBetContentField } from './betContentFormat';

describe('formatAdminBetContent', () => {
  it('formats position based lottery selections from objects', () => {
    expect(
      formatAdminBetContent({
        play_name: '前 3 直选',
        positions: [
          { position: 1, numbers: ['0', '1', '2'] },
          { position: 2, selected_numbers: '3,4,5' },
          { label: '第 3 位', values: [6, 7, 8, 9] }
        ]
      })
    ).toBe('玩法：前 3 直选；第 1 位：0、1、2；第 2 位：3、4、5；第 3 位：6、7、8、9');
  });

  it('parses JSON string bet content before formatting', () => {
    expect(formatAdminBetContent('[{"position":1,"numbers":["0","1"]},{"position":2,"numbers":["8","9"]}]')).toBe(
      '第 1 位：0、1；第 2 位：8、9'
    );
  });

  it('recognizes betting fields by key or Chinese label', () => {
    expect(isAdminBetContentField('bet_content')).toBe(true);
    expect(isAdminBetContentField('content', '投注内容')).toBe(true);
    expect(isAdminBetContentField('content_json', '新闻内容')).toBe(false);
  });
});
