-- Add a Chinese remark field for country configurations and backfill local display names for existing seed rows.
SET @add_country_remark_column := (
    SELECT IF(
        COUNT(*) = 0,
        'ALTER TABLE country_configs ADD COLUMN remark VARCHAR(128) NOT NULL DEFAULT '''' COMMENT ''中文国家或地区名称备注'' AFTER country_name',
        'SELECT 1'
    )
    FROM information_schema.COLUMNS
    WHERE TABLE_SCHEMA = DATABASE()
      AND TABLE_NAME = 'country_configs'
      AND COLUMN_NAME = 'remark'
);
PREPARE add_country_remark_column_stmt FROM @add_country_remark_column;
EXECUTE add_country_remark_column_stmt;
DEALLOCATE PREPARE add_country_remark_column_stmt;

UPDATE country_configs AS country
JOIN (
    SELECT 'AD' AS country_code, 'Andorra' AS country_name, 'Andorra' AS previous_country_name, '安道尔' AS remark
    UNION ALL SELECT 'AE' AS country_code, 'الإمارات العربية المتحدة' AS country_name, 'United Arab Emirates' AS previous_country_name, '阿拉伯联合酋长国' AS remark
    UNION ALL SELECT 'AF' AS country_code, 'افغانستان' AS country_name, 'Afghanistan' AS previous_country_name, '阿富汗' AS remark
    UNION ALL SELECT 'AG' AS country_code, 'Antigua & Barbuda' AS country_name, 'Antigua and Barbuda' AS previous_country_name, '安提瓜和巴布达' AS remark
    UNION ALL SELECT 'AI' AS country_code, 'Anguilla' AS country_name, 'Anguilla' AS previous_country_name, '安圭拉' AS remark
    UNION ALL SELECT 'AL' AS country_code, 'Shqipëri' AS country_name, 'Albania' AS previous_country_name, '阿尔巴尼亚' AS remark
    UNION ALL SELECT 'AM' AS country_code, 'Հայաստան' AS country_name, 'Armenia' AS previous_country_name, '亚美尼亚' AS remark
    UNION ALL SELECT 'AO' AS country_code, 'Angola' AS country_name, 'Angola' AS previous_country_name, '安哥拉' AS remark
    UNION ALL SELECT 'AQ' AS country_code, 'Antarctica' AS country_name, 'Antarctica' AS previous_country_name, '南极洲' AS remark
    UNION ALL SELECT 'AR' AS country_code, 'Argentina' AS country_name, 'Argentina' AS previous_country_name, '阿根廷' AS remark
    UNION ALL SELECT 'AS' AS country_code, 'American Samoa' AS country_name, 'American Samoa' AS previous_country_name, '美属萨摩亚' AS remark
    UNION ALL SELECT 'AT' AS country_code, 'Österreich' AS country_name, 'Austria' AS previous_country_name, '奥地利' AS remark
    UNION ALL SELECT 'AU' AS country_code, 'Australia' AS country_name, 'Australia' AS previous_country_name, '澳大利亚' AS remark
    UNION ALL SELECT 'AW' AS country_code, 'Aruba' AS country_name, 'Aruba' AS previous_country_name, '阿鲁巴' AS remark
    UNION ALL SELECT 'AX' AS country_code, 'Åland' AS country_name, 'Aland Islands' AS previous_country_name, '奥兰群岛' AS remark
    UNION ALL SELECT 'AZ' AS country_code, 'Azərbaycan' AS country_name, 'Azerbaijan' AS previous_country_name, '阿塞拜疆' AS remark
    UNION ALL SELECT 'BA' AS country_code, 'Bosna i Hercegovina' AS country_name, 'Bosnia and Herzegovina' AS previous_country_name, '波斯尼亚和黑塞哥维那' AS remark
    UNION ALL SELECT 'BB' AS country_code, 'Barbados' AS country_name, 'Barbados' AS previous_country_name, '巴巴多斯' AS remark
    UNION ALL SELECT 'BD' AS country_code, 'বাংলাদেশ' AS country_name, 'Bangladesh' AS previous_country_name, '孟加拉国' AS remark
    UNION ALL SELECT 'BE' AS country_code, 'België' AS country_name, 'Belgium' AS previous_country_name, '比利时' AS remark
    UNION ALL SELECT 'BF' AS country_code, 'Burkina Faso' AS country_name, 'Burkina Faso' AS previous_country_name, '布基纳法索' AS remark
    UNION ALL SELECT 'BG' AS country_code, 'България' AS country_name, 'Bulgaria' AS previous_country_name, '保加利亚' AS remark
    UNION ALL SELECT 'BH' AS country_code, 'البحرين' AS country_name, 'Bahrain' AS previous_country_name, '巴林' AS remark
    UNION ALL SELECT 'BI' AS country_code, 'Uburundi' AS country_name, 'Burundi' AS previous_country_name, '布隆迪' AS remark
    UNION ALL SELECT 'BJ' AS country_code, 'Bénin' AS country_name, 'Benin' AS previous_country_name, '贝宁' AS remark
    UNION ALL SELECT 'BL' AS country_code, 'Saint-Barthélemy' AS country_name, 'Saint Barthelemy' AS previous_country_name, '圣巴泰勒米' AS remark
    UNION ALL SELECT 'BM' AS country_code, 'Bermuda' AS country_name, 'Bermuda' AS previous_country_name, '百慕大' AS remark
    UNION ALL SELECT 'BN' AS country_code, 'Brunei' AS country_name, 'Brunei Darussalam' AS previous_country_name, '文莱' AS remark
    UNION ALL SELECT 'BO' AS country_code, 'Bolivia' AS country_name, 'Bolivia' AS previous_country_name, '玻利维亚' AS remark
    UNION ALL SELECT 'BQ' AS country_code, 'Caribisch Nederland' AS country_name, 'Bonaire, Sint Eustatius and Saba' AS previous_country_name, '荷属加勒比区' AS remark
    UNION ALL SELECT 'BR' AS country_code, 'Brasil' AS country_name, 'Brazil' AS previous_country_name, '巴西' AS remark
    UNION ALL SELECT 'BS' AS country_code, 'Bahamas' AS country_name, 'Bahamas' AS previous_country_name, '巴哈马' AS remark
    UNION ALL SELECT 'BT' AS country_code, 'འབྲུག' AS country_name, 'Bhutan' AS previous_country_name, '不丹' AS remark
    UNION ALL SELECT 'BV' AS country_code, 'Bouvetøya' AS country_name, 'Bouvet Island' AS previous_country_name, '布韦岛' AS remark
    UNION ALL SELECT 'BW' AS country_code, 'Botswana' AS country_name, 'Botswana' AS previous_country_name, '博茨瓦纳' AS remark
    UNION ALL SELECT 'BY' AS country_code, 'Беларусь' AS country_name, 'Belarus' AS previous_country_name, '白俄罗斯' AS remark
    UNION ALL SELECT 'BZ' AS country_code, 'Belize' AS country_name, 'Belize' AS previous_country_name, '伯利兹' AS remark
    UNION ALL SELECT 'CA' AS country_code, 'Canada' AS country_name, 'Canada' AS previous_country_name, '加拿大' AS remark
    UNION ALL SELECT 'CC' AS country_code, 'Cocos (Keeling) Islands' AS country_name, 'Cocos Islands' AS previous_country_name, '科科斯（基林）群岛' AS remark
    UNION ALL SELECT 'CD' AS country_code, 'Congo-Kinshasa' AS country_name, 'Congo, Democratic Republic of the' AS previous_country_name, '刚果（金）' AS remark
    UNION ALL SELECT 'CF' AS country_code, 'Ködörösêse tî Bêafrîka' AS country_name, 'Central African Republic' AS previous_country_name, '中非共和国' AS remark
    UNION ALL SELECT 'CG' AS country_code, 'Congo-Brazzaville' AS country_name, 'Congo' AS previous_country_name, '刚果（布）' AS remark
    UNION ALL SELECT 'CH' AS country_code, 'Schweiz' AS country_name, 'Switzerland' AS previous_country_name, '瑞士' AS remark
    UNION ALL SELECT 'CI' AS country_code, 'Côte d’Ivoire' AS country_name, 'Cote d''Ivoire' AS previous_country_name, '科特迪瓦' AS remark
    UNION ALL SELECT 'CK' AS country_code, 'Cook Islands' AS country_name, 'Cook Islands' AS previous_country_name, '库克群岛' AS remark
    UNION ALL SELECT 'CL' AS country_code, 'Chile' AS country_name, 'Chile' AS previous_country_name, '智利' AS remark
    UNION ALL SELECT 'CM' AS country_code, 'Cameroun' AS country_name, 'Cameroon' AS previous_country_name, '喀麦隆' AS remark
    UNION ALL SELECT 'CN' AS country_code, '中国' AS country_name, 'China' AS previous_country_name, '中国' AS remark
    UNION ALL SELECT 'CO' AS country_code, 'Colombia' AS country_name, 'Colombia' AS previous_country_name, '哥伦比亚' AS remark
    UNION ALL SELECT 'CR' AS country_code, 'Costa Rica' AS country_name, 'Costa Rica' AS previous_country_name, '哥斯达黎加' AS remark
    UNION ALL SELECT 'CU' AS country_code, 'Cuba' AS country_name, 'Cuba' AS previous_country_name, '古巴' AS remark
    UNION ALL SELECT 'CV' AS country_code, 'Cabo Verde' AS country_name, 'Cabo Verde' AS previous_country_name, '佛得角' AS remark
    UNION ALL SELECT 'CW' AS country_code, 'Curaçao' AS country_name, 'Curacao' AS previous_country_name, '库拉索' AS remark
    UNION ALL SELECT 'CX' AS country_code, 'Christmas Island' AS country_name, 'Christmas Island' AS previous_country_name, '圣诞岛' AS remark
    UNION ALL SELECT 'CY' AS country_code, 'Κύπρος' AS country_name, 'Cyprus' AS previous_country_name, '塞浦路斯' AS remark
    UNION ALL SELECT 'CZ' AS country_code, 'Česko' AS country_name, 'Czechia' AS previous_country_name, '捷克' AS remark
    UNION ALL SELECT 'DE' AS country_code, 'Deutschland' AS country_name, 'Germany' AS previous_country_name, '德国' AS remark
    UNION ALL SELECT 'DJ' AS country_code, 'Djibouti' AS country_name, 'Djibouti' AS previous_country_name, '吉布提' AS remark
    UNION ALL SELECT 'DK' AS country_code, 'Danmark' AS country_name, 'Denmark' AS previous_country_name, '丹麦' AS remark
    UNION ALL SELECT 'DM' AS country_code, 'Dominica' AS country_name, 'Dominica' AS previous_country_name, '多米尼克' AS remark
    UNION ALL SELECT 'DO' AS country_code, 'República Dominicana' AS country_name, 'Dominican Republic' AS previous_country_name, '多米尼加共和国' AS remark
    UNION ALL SELECT 'DZ' AS country_code, 'الجزائر' AS country_name, 'Algeria' AS previous_country_name, '阿尔及利亚' AS remark
    UNION ALL SELECT 'EC' AS country_code, 'Ecuador' AS country_name, 'Ecuador' AS previous_country_name, '厄瓜多尔' AS remark
    UNION ALL SELECT 'EE' AS country_code, 'Eesti' AS country_name, 'Estonia' AS previous_country_name, '爱沙尼亚' AS remark
    UNION ALL SELECT 'EG' AS country_code, 'مصر' AS country_name, 'Egypt' AS previous_country_name, '埃及' AS remark
    UNION ALL SELECT 'EH' AS country_code, 'الصحراء الغربية' AS country_name, 'Western Sahara' AS previous_country_name, '西撒哈拉' AS remark
    UNION ALL SELECT 'ER' AS country_code, 'Eritrea' AS country_name, 'Eritrea' AS previous_country_name, '厄立特里亚' AS remark
    UNION ALL SELECT 'ES' AS country_code, 'España' AS country_name, 'Spain' AS previous_country_name, '西班牙' AS remark
    UNION ALL SELECT 'ET' AS country_code, 'ኢትዮጵያ' AS country_name, 'Ethiopia' AS previous_country_name, '埃塞俄比亚' AS remark
    UNION ALL SELECT 'FI' AS country_code, 'Suomi' AS country_name, 'Finland' AS previous_country_name, '芬兰' AS remark
    UNION ALL SELECT 'FJ' AS country_code, 'Fiji' AS country_name, 'Fiji' AS previous_country_name, '斐济' AS remark
    UNION ALL SELECT 'FK' AS country_code, 'Falkland Islands' AS country_name, 'Falkland Islands' AS previous_country_name, '福克兰群岛' AS remark
    UNION ALL SELECT 'FM' AS country_code, 'Micronesia' AS country_name, 'Micronesia' AS previous_country_name, '密克罗尼西亚' AS remark
    UNION ALL SELECT 'FO' AS country_code, 'Føroyar' AS country_name, 'Faroe Islands' AS previous_country_name, '法罗群岛' AS remark
    UNION ALL SELECT 'FR' AS country_code, 'France' AS country_name, 'France' AS previous_country_name, '法国' AS remark
    UNION ALL SELECT 'GA' AS country_code, 'Gabon' AS country_name, 'Gabon' AS previous_country_name, '加蓬' AS remark
    UNION ALL SELECT 'GB' AS country_code, 'United Kingdom' AS country_name, 'United Kingdom' AS previous_country_name, '英国' AS remark
    UNION ALL SELECT 'GD' AS country_code, 'Grenada' AS country_name, 'Grenada' AS previous_country_name, '格林纳达' AS remark
    UNION ALL SELECT 'GE' AS country_code, 'საქართველო' AS country_name, 'Georgia' AS previous_country_name, '格鲁吉亚' AS remark
    UNION ALL SELECT 'GF' AS country_code, 'Guyane française' AS country_name, 'French Guiana' AS previous_country_name, '法属圭亚那' AS remark
    UNION ALL SELECT 'GG' AS country_code, 'Guernsey' AS country_name, 'Guernsey' AS previous_country_name, '根西岛' AS remark
    UNION ALL SELECT 'GH' AS country_code, 'Ghana' AS country_name, 'Ghana' AS previous_country_name, '加纳' AS remark
    UNION ALL SELECT 'GI' AS country_code, 'Gibraltar' AS country_name, 'Gibraltar' AS previous_country_name, '直布罗陀' AS remark
    UNION ALL SELECT 'GL' AS country_code, 'Kalaallit Nunaat' AS country_name, 'Greenland' AS previous_country_name, '格陵兰' AS remark
    UNION ALL SELECT 'GM' AS country_code, 'Gambia' AS country_name, 'Gambia' AS previous_country_name, '冈比亚' AS remark
    UNION ALL SELECT 'GN' AS country_code, 'Guinée' AS country_name, 'Guinea' AS previous_country_name, '几内亚' AS remark
    UNION ALL SELECT 'GP' AS country_code, 'Guadeloupe' AS country_name, 'Guadeloupe' AS previous_country_name, '瓜德罗普' AS remark
    UNION ALL SELECT 'GQ' AS country_code, 'Guinea Ecuatorial' AS country_name, 'Equatorial Guinea' AS previous_country_name, '赤道几内亚' AS remark
    UNION ALL SELECT 'GR' AS country_code, 'Ελλάδα' AS country_name, 'Greece' AS previous_country_name, '希腊' AS remark
    UNION ALL SELECT 'GS' AS country_code, 'South Georgia & South Sandwich Islands' AS country_name, 'South Georgia and the South Sandwich Islands' AS previous_country_name, '南乔治亚和南桑威奇群岛' AS remark
    UNION ALL SELECT 'GT' AS country_code, 'Guatemala' AS country_name, 'Guatemala' AS previous_country_name, '危地马拉' AS remark
    UNION ALL SELECT 'GU' AS country_code, 'Guam' AS country_name, 'Guam' AS previous_country_name, '关岛' AS remark
    UNION ALL SELECT 'GW' AS country_code, 'Guiné-Bissau' AS country_name, 'Guinea-Bissau' AS previous_country_name, '几内亚比绍' AS remark
    UNION ALL SELECT 'GY' AS country_code, 'Guyana' AS country_name, 'Guyana' AS previous_country_name, '圭亚那' AS remark
    UNION ALL SELECT 'HK' AS country_code, '中國香港特別行政區' AS country_name, 'Hong Kong' AS previous_country_name, '中国香港特别行政区' AS remark
    UNION ALL SELECT 'HM' AS country_code, 'Heard & McDonald Islands' AS country_name, 'Heard Island and McDonald Islands' AS previous_country_name, '赫德岛和麦克唐纳群岛' AS remark
    UNION ALL SELECT 'HN' AS country_code, 'Honduras' AS country_name, 'Honduras' AS previous_country_name, '洪都拉斯' AS remark
    UNION ALL SELECT 'HR' AS country_code, 'Hrvatska' AS country_name, 'Croatia' AS previous_country_name, '克罗地亚' AS remark
    UNION ALL SELECT 'HT' AS country_code, 'Haiti' AS country_name, 'Haiti' AS previous_country_name, '海地' AS remark
    UNION ALL SELECT 'HU' AS country_code, 'Magyarország' AS country_name, 'Hungary' AS previous_country_name, '匈牙利' AS remark
    UNION ALL SELECT 'ID' AS country_code, 'Indonesia' AS country_name, 'Indonesia' AS previous_country_name, '印度尼西亚' AS remark
    UNION ALL SELECT 'IE' AS country_code, 'Ireland' AS country_name, 'Ireland' AS previous_country_name, '爱尔兰' AS remark
    UNION ALL SELECT 'IL' AS country_code, 'ישראל' AS country_name, 'Israel' AS previous_country_name, '以色列' AS remark
    UNION ALL SELECT 'IM' AS country_code, 'Isle of Man' AS country_name, 'Isle of Man' AS previous_country_name, '马恩岛' AS remark
    UNION ALL SELECT 'IN' AS country_code, 'भारत' AS country_name, 'India' AS previous_country_name, '印度' AS remark
    UNION ALL SELECT 'IO' AS country_code, 'British Indian Ocean Territory' AS country_name, 'British Indian Ocean Territory' AS previous_country_name, '英属印度洋领地' AS remark
    UNION ALL SELECT 'IQ' AS country_code, 'العراق' AS country_name, 'Iraq' AS previous_country_name, '伊拉克' AS remark
    UNION ALL SELECT 'IR' AS country_code, 'ایران' AS country_name, 'Iran' AS previous_country_name, '伊朗' AS remark
    UNION ALL SELECT 'IS' AS country_code, 'Ísland' AS country_name, 'Iceland' AS previous_country_name, '冰岛' AS remark
    UNION ALL SELECT 'IT' AS country_code, 'Italia' AS country_name, 'Italy' AS previous_country_name, '意大利' AS remark
    UNION ALL SELECT 'JE' AS country_code, 'Jersey' AS country_name, 'Jersey' AS previous_country_name, '泽西岛' AS remark
    UNION ALL SELECT 'JM' AS country_code, 'Jamaica' AS country_name, 'Jamaica' AS previous_country_name, '牙买加' AS remark
    UNION ALL SELECT 'JO' AS country_code, 'الأردن' AS country_name, 'Jordan' AS previous_country_name, '约旦' AS remark
    UNION ALL SELECT 'JP' AS country_code, '日本' AS country_name, 'Japan' AS previous_country_name, '日本' AS remark
    UNION ALL SELECT 'KE' AS country_code, 'Kenya' AS country_name, 'Kenya' AS previous_country_name, '肯尼亚' AS remark
    UNION ALL SELECT 'KG' AS country_code, 'Кыргызстан' AS country_name, 'Kyrgyzstan' AS previous_country_name, '吉尔吉斯斯坦' AS remark
    UNION ALL SELECT 'KH' AS country_code, 'កម្ពុជា' AS country_name, 'Cambodia' AS previous_country_name, '柬埔寨' AS remark
    UNION ALL SELECT 'KI' AS country_code, 'Kiribati' AS country_name, 'Kiribati' AS previous_country_name, '基里巴斯' AS remark
    UNION ALL SELECT 'KM' AS country_code, 'جزر القمر' AS country_name, 'Comoros' AS previous_country_name, '科摩罗' AS remark
    UNION ALL SELECT 'KN' AS country_code, 'St. Kitts & Nevis' AS country_name, 'Saint Kitts and Nevis' AS previous_country_name, '圣基茨和尼维斯' AS remark
    UNION ALL SELECT 'KP' AS country_code, '북한' AS country_name, 'North Korea' AS previous_country_name, '朝鲜' AS remark
    UNION ALL SELECT 'KR' AS country_code, '대한민국' AS country_name, 'South Korea' AS previous_country_name, '韩国' AS remark
    UNION ALL SELECT 'KW' AS country_code, 'الكويت' AS country_name, 'Kuwait' AS previous_country_name, '科威特' AS remark
    UNION ALL SELECT 'KY' AS country_code, 'Cayman Islands' AS country_name, 'Cayman Islands' AS previous_country_name, '开曼群岛' AS remark
    UNION ALL SELECT 'KZ' AS country_code, 'Казахстан' AS country_name, 'Kazakhstan' AS previous_country_name, '哈萨克斯坦' AS remark
    UNION ALL SELECT 'LA' AS country_code, 'ລາວ' AS country_name, 'Laos' AS previous_country_name, '老挝' AS remark
    UNION ALL SELECT 'LB' AS country_code, 'لبنان' AS country_name, 'Lebanon' AS previous_country_name, '黎巴嫩' AS remark
    UNION ALL SELECT 'LC' AS country_code, 'St. Lucia' AS country_name, 'Saint Lucia' AS previous_country_name, '圣卢西亚' AS remark
    UNION ALL SELECT 'LI' AS country_code, 'Liechtenstein' AS country_name, 'Liechtenstein' AS previous_country_name, '列支敦士登' AS remark
    UNION ALL SELECT 'LK' AS country_code, 'ශ්‍රී ලංකාව' AS country_name, 'Sri Lanka' AS previous_country_name, '斯里兰卡' AS remark
    UNION ALL SELECT 'LR' AS country_code, 'Liberia' AS country_name, 'Liberia' AS previous_country_name, '利比里亚' AS remark
    UNION ALL SELECT 'LS' AS country_code, 'Lesotho' AS country_name, 'Lesotho' AS previous_country_name, '莱索托' AS remark
    UNION ALL SELECT 'LT' AS country_code, 'Lietuva' AS country_name, 'Lithuania' AS previous_country_name, '立陶宛' AS remark
    UNION ALL SELECT 'LU' AS country_code, 'Luxembourg' AS country_name, 'Luxembourg' AS previous_country_name, '卢森堡' AS remark
    UNION ALL SELECT 'LV' AS country_code, 'Latvija' AS country_name, 'Latvia' AS previous_country_name, '拉脱维亚' AS remark
    UNION ALL SELECT 'LY' AS country_code, 'ليبيا' AS country_name, 'Libya' AS previous_country_name, '利比亚' AS remark
    UNION ALL SELECT 'MA' AS country_code, 'المغرب' AS country_name, 'Morocco' AS previous_country_name, '摩洛哥' AS remark
    UNION ALL SELECT 'MC' AS country_code, 'Monaco' AS country_name, 'Monaco' AS previous_country_name, '摩纳哥' AS remark
    UNION ALL SELECT 'MD' AS country_code, 'Republica Moldova' AS country_name, 'Moldova' AS previous_country_name, '摩尔多瓦' AS remark
    UNION ALL SELECT 'ME' AS country_code, 'Crna Gora' AS country_name, 'Montenegro' AS previous_country_name, '黑山' AS remark
    UNION ALL SELECT 'MF' AS country_code, 'Saint-Martin' AS country_name, 'Saint Martin' AS previous_country_name, '法属圣马丁' AS remark
    UNION ALL SELECT 'MG' AS country_code, 'Madagasikara' AS country_name, 'Madagascar' AS previous_country_name, '马达加斯加' AS remark
    UNION ALL SELECT 'MH' AS country_code, 'Marshall Islands' AS country_name, 'Marshall Islands' AS previous_country_name, '马绍尔群岛' AS remark
    UNION ALL SELECT 'MK' AS country_code, 'Северна Македонија' AS country_name, 'North Macedonia' AS previous_country_name, '北马其顿' AS remark
    UNION ALL SELECT 'ML' AS country_code, 'Mali' AS country_name, 'Mali' AS previous_country_name, '马里' AS remark
    UNION ALL SELECT 'MM' AS country_code, 'မြန်မာ' AS country_name, 'Myanmar' AS previous_country_name, '缅甸' AS remark
    UNION ALL SELECT 'MN' AS country_code, 'Монгол' AS country_name, 'Mongolia' AS previous_country_name, '蒙古' AS remark
    UNION ALL SELECT 'MO' AS country_code, '中國澳門特別行政區' AS country_name, 'Macao' AS previous_country_name, '中国澳门特别行政区' AS remark
    UNION ALL SELECT 'MP' AS country_code, 'Northern Mariana Islands' AS country_name, 'Northern Mariana Islands' AS previous_country_name, '北马里亚纳群岛' AS remark
    UNION ALL SELECT 'MQ' AS country_code, 'Martinique' AS country_name, 'Martinique' AS previous_country_name, '马提尼克' AS remark
    UNION ALL SELECT 'MR' AS country_code, 'موريتانيا' AS country_name, 'Mauritania' AS previous_country_name, '毛里塔尼亚' AS remark
    UNION ALL SELECT 'MS' AS country_code, 'Montserrat' AS country_name, 'Montserrat' AS previous_country_name, '蒙特塞拉特' AS remark
    UNION ALL SELECT 'MT' AS country_code, 'Malta' AS country_name, 'Malta' AS previous_country_name, '马耳他' AS remark
    UNION ALL SELECT 'MU' AS country_code, 'Maurice' AS country_name, 'Mauritius' AS previous_country_name, '毛里求斯' AS remark
    UNION ALL SELECT 'MV' AS country_code, 'Maldives' AS country_name, 'Maldives' AS previous_country_name, '马尔代夫' AS remark
    UNION ALL SELECT 'MW' AS country_code, 'Malawi' AS country_name, 'Malawi' AS previous_country_name, '马拉维' AS remark
    UNION ALL SELECT 'MX' AS country_code, 'México' AS country_name, 'Mexico' AS previous_country_name, '墨西哥' AS remark
    UNION ALL SELECT 'MY' AS country_code, 'Malaysia' AS country_name, 'Malaysia' AS previous_country_name, '马来西亚' AS remark
    UNION ALL SELECT 'MZ' AS country_code, 'Moçambique' AS country_name, 'Mozambique' AS previous_country_name, '莫桑比克' AS remark
    UNION ALL SELECT 'NA' AS country_code, 'Namibia' AS country_name, 'Namibia' AS previous_country_name, '纳米比亚' AS remark
    UNION ALL SELECT 'NC' AS country_code, 'Nouvelle-Calédonie' AS country_name, 'New Caledonia' AS previous_country_name, '新喀里多尼亚' AS remark
    UNION ALL SELECT 'NE' AS country_code, 'Niger' AS country_name, 'Niger' AS previous_country_name, '尼日尔' AS remark
    UNION ALL SELECT 'NF' AS country_code, 'Norfolk Island' AS country_name, 'Norfolk Island' AS previous_country_name, '诺福克岛' AS remark
    UNION ALL SELECT 'NG' AS country_code, 'Nigeria' AS country_name, 'Nigeria' AS previous_country_name, '尼日利亚' AS remark
    UNION ALL SELECT 'NI' AS country_code, 'Nicaragua' AS country_name, 'Nicaragua' AS previous_country_name, '尼加拉瓜' AS remark
    UNION ALL SELECT 'NL' AS country_code, 'Nederland' AS country_name, 'Netherlands' AS previous_country_name, '荷兰' AS remark
    UNION ALL SELECT 'NO' AS country_code, 'Norge' AS country_name, 'Norway' AS previous_country_name, '挪威' AS remark
    UNION ALL SELECT 'NP' AS country_code, 'नेपाल' AS country_name, 'Nepal' AS previous_country_name, '尼泊尔' AS remark
    UNION ALL SELECT 'NR' AS country_code, 'Nauru' AS country_name, 'Nauru' AS previous_country_name, '瑙鲁' AS remark
    UNION ALL SELECT 'NU' AS country_code, 'Niue' AS country_name, 'Niue' AS previous_country_name, '纽埃' AS remark
    UNION ALL SELECT 'NZ' AS country_code, 'Aotearoa' AS country_name, 'New Zealand' AS previous_country_name, '新西兰' AS remark
    UNION ALL SELECT 'OM' AS country_code, 'عُمان' AS country_name, 'Oman' AS previous_country_name, '阿曼' AS remark
    UNION ALL SELECT 'PA' AS country_code, 'Panamá' AS country_name, 'Panama' AS previous_country_name, '巴拿马' AS remark
    UNION ALL SELECT 'PE' AS country_code, 'Perú' AS country_name, 'Peru' AS previous_country_name, '秘鲁' AS remark
    UNION ALL SELECT 'PF' AS country_code, 'Polynésie française' AS country_name, 'French Polynesia' AS previous_country_name, '法属波利尼西亚' AS remark
    UNION ALL SELECT 'PG' AS country_code, 'Papua New Guinea' AS country_name, 'Papua New Guinea' AS previous_country_name, '巴布亚新几内亚' AS remark
    UNION ALL SELECT 'PH' AS country_code, 'Philippines' AS country_name, 'Philippines' AS previous_country_name, '菲律宾' AS remark
    UNION ALL SELECT 'PK' AS country_code, 'پاکستان' AS country_name, 'Pakistan' AS previous_country_name, '巴基斯坦' AS remark
    UNION ALL SELECT 'PL' AS country_code, 'Polska' AS country_name, 'Poland' AS previous_country_name, '波兰' AS remark
    UNION ALL SELECT 'PM' AS country_code, 'Saint-Pierre-et-Miquelon' AS country_name, 'Saint Pierre and Miquelon' AS previous_country_name, '圣皮埃尔和密克隆群岛' AS remark
    UNION ALL SELECT 'PN' AS country_code, 'Pitcairn Islands' AS country_name, 'Pitcairn' AS previous_country_name, '皮特凯恩群岛' AS remark
    UNION ALL SELECT 'PR' AS country_code, 'Puerto Rico' AS country_name, 'Puerto Rico' AS previous_country_name, '波多黎各' AS remark
    UNION ALL SELECT 'PS' AS country_code, 'الأراضي الفلسطينية' AS country_name, 'Palestine' AS previous_country_name, '巴勒斯坦领土' AS remark
    UNION ALL SELECT 'PT' AS country_code, 'Portugal' AS country_name, 'Portugal' AS previous_country_name, '葡萄牙' AS remark
    UNION ALL SELECT 'PW' AS country_code, 'Palau' AS country_name, 'Palau' AS previous_country_name, '帕劳' AS remark
    UNION ALL SELECT 'PY' AS country_code, 'Paraguay' AS country_name, 'Paraguay' AS previous_country_name, '巴拉圭' AS remark
    UNION ALL SELECT 'QA' AS country_code, 'قطر' AS country_name, 'Qatar' AS previous_country_name, '卡塔尔' AS remark
    UNION ALL SELECT 'RE' AS country_code, 'La Réunion' AS country_name, 'Reunion' AS previous_country_name, '留尼汪' AS remark
    UNION ALL SELECT 'RO' AS country_code, 'România' AS country_name, 'Romania' AS previous_country_name, '罗马尼亚' AS remark
    UNION ALL SELECT 'RS' AS country_code, 'Србија' AS country_name, 'Serbia' AS previous_country_name, '塞尔维亚' AS remark
    UNION ALL SELECT 'RU' AS country_code, 'Россия' AS country_name, 'Russia' AS previous_country_name, '俄罗斯' AS remark
    UNION ALL SELECT 'RW' AS country_code, 'U Rwanda' AS country_name, 'Rwanda' AS previous_country_name, '卢旺达' AS remark
    UNION ALL SELECT 'SA' AS country_code, 'المملكة العربية السعودية' AS country_name, 'Saudi Arabia' AS previous_country_name, '沙特阿拉伯' AS remark
    UNION ALL SELECT 'SB' AS country_code, 'Solomon Islands' AS country_name, 'Solomon Islands' AS previous_country_name, '所罗门群岛' AS remark
    UNION ALL SELECT 'SC' AS country_code, 'Seychelles' AS country_name, 'Seychelles' AS previous_country_name, '塞舌尔' AS remark
    UNION ALL SELECT 'SD' AS country_code, 'السودان' AS country_name, 'Sudan' AS previous_country_name, '苏丹' AS remark
    UNION ALL SELECT 'SE' AS country_code, 'Sverige' AS country_name, 'Sweden' AS previous_country_name, '瑞典' AS remark
    UNION ALL SELECT 'SG' AS country_code, 'Singapore' AS country_name, 'Singapore' AS previous_country_name, '新加坡' AS remark
    UNION ALL SELECT 'SH' AS country_code, 'St. Helena' AS country_name, 'Saint Helena, Ascension and Tristan da Cunha' AS previous_country_name, '圣赫勒拿' AS remark
    UNION ALL SELECT 'SI' AS country_code, 'Slovenija' AS country_name, 'Slovenia' AS previous_country_name, '斯洛文尼亚' AS remark
    UNION ALL SELECT 'SJ' AS country_code, 'Svalbard og Jan Mayen' AS country_name, 'Svalbard and Jan Mayen' AS previous_country_name, '斯瓦尔巴和扬马延' AS remark
    UNION ALL SELECT 'SK' AS country_code, 'Slovensko' AS country_name, 'Slovakia' AS previous_country_name, '斯洛伐克' AS remark
    UNION ALL SELECT 'SL' AS country_code, 'Sierra Leone' AS country_name, 'Sierra Leone' AS previous_country_name, '塞拉利昂' AS remark
    UNION ALL SELECT 'SM' AS country_code, 'San Marino' AS country_name, 'San Marino' AS previous_country_name, '圣马力诺' AS remark
    UNION ALL SELECT 'SN' AS country_code, 'Sénégal' AS country_name, 'Senegal' AS previous_country_name, '塞内加尔' AS remark
    UNION ALL SELECT 'SO' AS country_code, 'Soomaaliya' AS country_name, 'Somalia' AS previous_country_name, '索马里' AS remark
    UNION ALL SELECT 'SR' AS country_code, 'Suriname' AS country_name, 'Suriname' AS previous_country_name, '苏里南' AS remark
    UNION ALL SELECT 'SS' AS country_code, 'South Sudan' AS country_name, 'South Sudan' AS previous_country_name, '南苏丹' AS remark
    UNION ALL SELECT 'ST' AS country_code, 'São Tomé e Príncipe' AS country_name, 'Sao Tome and Principe' AS previous_country_name, '圣多美和普林西比' AS remark
    UNION ALL SELECT 'SV' AS country_code, 'El Salvador' AS country_name, 'El Salvador' AS previous_country_name, '萨尔瓦多' AS remark
    UNION ALL SELECT 'SX' AS country_code, 'Sint Maarten' AS country_name, 'Sint Maarten' AS previous_country_name, '荷属圣马丁' AS remark
    UNION ALL SELECT 'SY' AS country_code, 'سوريا' AS country_name, 'Syria' AS previous_country_name, '叙利亚' AS remark
    UNION ALL SELECT 'SZ' AS country_code, 'Eswatini' AS country_name, 'Eswatini' AS previous_country_name, '斯威士兰' AS remark
    UNION ALL SELECT 'TC' AS country_code, 'Turks & Caicos Islands' AS country_name, 'Turks and Caicos Islands' AS previous_country_name, '特克斯和凯科斯群岛' AS remark
    UNION ALL SELECT 'TD' AS country_code, 'تشاد' AS country_name, 'Chad' AS previous_country_name, '乍得' AS remark
    UNION ALL SELECT 'TF' AS country_code, 'Terres australes françaises' AS country_name, 'French Southern Territories' AS previous_country_name, '法属南部领地' AS remark
    UNION ALL SELECT 'TG' AS country_code, 'Togo' AS country_name, 'Togo' AS previous_country_name, '多哥' AS remark
    UNION ALL SELECT 'TH' AS country_code, 'ไทย' AS country_name, 'Thailand' AS previous_country_name, '泰国' AS remark
    UNION ALL SELECT 'TJ' AS country_code, 'Тоҷикистон' AS country_name, 'Tajikistan' AS previous_country_name, '塔吉克斯坦' AS remark
    UNION ALL SELECT 'TK' AS country_code, 'Tokelau' AS country_name, 'Tokelau' AS previous_country_name, '托克劳' AS remark
    UNION ALL SELECT 'TL' AS country_code, 'Timor-Leste' AS country_name, 'Timor-Leste' AS previous_country_name, '东帝汶' AS remark
    UNION ALL SELECT 'TM' AS country_code, 'Türkmenistan' AS country_name, 'Turkmenistan' AS previous_country_name, '土库曼斯坦' AS remark
    UNION ALL SELECT 'TN' AS country_code, 'تونس' AS country_name, 'Tunisia' AS previous_country_name, '突尼斯' AS remark
    UNION ALL SELECT 'TO' AS country_code, 'Tonga' AS country_name, 'Tonga' AS previous_country_name, '汤加' AS remark
    UNION ALL SELECT 'TR' AS country_code, 'Türkiye' AS country_name, 'Turkey' AS previous_country_name, '土耳其' AS remark
    UNION ALL SELECT 'TT' AS country_code, 'Trinidad & Tobago' AS country_name, 'Trinidad and Tobago' AS previous_country_name, '特立尼达和多巴哥' AS remark
    UNION ALL SELECT 'TV' AS country_code, 'Tuvalu' AS country_name, 'Tuvalu' AS previous_country_name, '图瓦卢' AS remark
    UNION ALL SELECT 'TW' AS country_code, '台灣' AS country_name, 'Taiwan' AS previous_country_name, '台湾' AS remark
    UNION ALL SELECT 'TZ' AS country_code, 'Tanzania' AS country_name, 'Tanzania' AS previous_country_name, '坦桑尼亚' AS remark
    UNION ALL SELECT 'UA' AS country_code, 'Україна' AS country_name, 'Ukraine' AS previous_country_name, '乌克兰' AS remark
    UNION ALL SELECT 'UG' AS country_code, 'Uganda' AS country_name, 'Uganda' AS previous_country_name, '乌干达' AS remark
    UNION ALL SELECT 'UM' AS country_code, 'U.S. Outlying Islands' AS country_name, 'United States Minor Outlying Islands' AS previous_country_name, '美国本土外小岛屿' AS remark
    UNION ALL SELECT 'US' AS country_code, 'United States' AS country_name, 'United States' AS previous_country_name, '美国' AS remark
    UNION ALL SELECT 'UY' AS country_code, 'Uruguay' AS country_name, 'Uruguay' AS previous_country_name, '乌拉圭' AS remark
    UNION ALL SELECT 'UZ' AS country_code, 'Oʻzbekiston' AS country_name, 'Uzbekistan' AS previous_country_name, '乌兹别克斯坦' AS remark
    UNION ALL SELECT 'VA' AS country_code, 'Città del Vaticano' AS country_name, 'Vatican City' AS previous_country_name, '梵蒂冈' AS remark
    UNION ALL SELECT 'VC' AS country_code, 'St. Vincent & Grenadines' AS country_name, 'Saint Vincent and the Grenadines' AS previous_country_name, '圣文森特和格林纳丁斯' AS remark
    UNION ALL SELECT 'VE' AS country_code, 'Venezuela' AS country_name, 'Venezuela' AS previous_country_name, '委内瑞拉' AS remark
    UNION ALL SELECT 'VG' AS country_code, 'British Virgin Islands' AS country_name, 'Virgin Islands, British' AS previous_country_name, '英属维尔京群岛' AS remark
    UNION ALL SELECT 'VI' AS country_code, 'U.S. Virgin Islands' AS country_name, 'Virgin Islands, U.S.' AS previous_country_name, '美属维尔京群岛' AS remark
    UNION ALL SELECT 'VN' AS country_code, 'Việt Nam' AS country_name, 'Vietnam' AS previous_country_name, '越南' AS remark
    UNION ALL SELECT 'VU' AS country_code, 'Vanuatu' AS country_name, 'Vanuatu' AS previous_country_name, '瓦努阿图' AS remark
    UNION ALL SELECT 'WF' AS country_code, 'Wallis-et-Futuna' AS country_name, 'Wallis and Futuna' AS previous_country_name, '瓦利斯和富图纳' AS remark
    UNION ALL SELECT 'WS' AS country_code, 'Samoa' AS country_name, 'Samoa' AS previous_country_name, '萨摩亚' AS remark
    UNION ALL SELECT 'YE' AS country_code, 'اليمن' AS country_name, 'Yemen' AS previous_country_name, '也门' AS remark
    UNION ALL SELECT 'YT' AS country_code, 'Mayotte' AS country_name, 'Mayotte' AS previous_country_name, '马约特' AS remark
    UNION ALL SELECT 'ZA' AS country_code, 'South Africa' AS country_name, 'South Africa' AS previous_country_name, '南非' AS remark
    UNION ALL SELECT 'ZM' AS country_code, 'Zambia' AS country_name, 'Zambia' AS previous_country_name, '赞比亚' AS remark
    UNION ALL SELECT 'ZW' AS country_code, 'Zimbabwe' AS country_name, 'Zimbabwe' AS previous_country_name, '津巴布韦' AS remark
) AS seed ON seed.country_code = country.country_code
SET country.country_name = CASE
        WHEN country.country_name = seed.previous_country_name OR country.country_name = '' THEN seed.country_name
        ELSE country.country_name
    END,
    country.remark = CASE
        WHEN country.remark IS NULL OR country.remark = '' THEN seed.remark
        ELSE country.remark
    END;
