//! Mobile Country Code (MCC)
//! - Consists of 3 digits.
//! - Example: '460' (China)
//! - For a detailed list, please refer to: https://www.mcc-mnc.com/
//!
//! ---
//!
//! 移动国家码 (Mobile Country Code, MCC)
//! - 由3位数字组成。
//! - 例如: '460' (中国)
//! - 详细列表请查阅: https://www.mcc-mnc.com/

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[allow(non_camel_case_types)]
#[derive(EnumIter)]
pub enum MCC {
    /// 🇦🇫 AF Afghanistan
    ///
    /// ---
    /// AF 阿富汗
    AF = 412,
    /// 🇦🇱 AL Albania
    ///
    /// ---
    /// AL 阿尔巴尼亚
    AL = 276,
    /// 🇩🇿 DZ Algeria
    ///
    /// ---
    /// DZ 阿尔及利亚
    DZ = 603,
    /// 🇦🇸 AS American Samoa
    ///
    /// ---
    /// AS 美属萨摩亚 (美国)
    AS = 544,
    /// 🇦🇩 AD Andorra
    ///
    /// ---
    /// AD 安道尔
    AD = 213,
    /// 🇦🇴 AO Angola
    ///
    /// ---
    /// AO 安哥拉
    AO = 631,
    /// 🇦🇮 AI Anguilla
    ///
    /// ---
    /// AI 安圭拉
    AI = 365,
    /// 🇦🇬 AG Antigua and Barbuda
    ///
    /// ---
    /// AG 安提瓜和巴布达
    AG = 344,
    /// 🇦🇷 AR Argentina
    ///
    /// ---
    /// AR 阿根廷
    AR = 722,
    /// 🇦🇲 AM Armenia
    ///
    /// ---
    /// AM 亚美尼亚
    AM = 283,
    /// 🇦🇼 AW Aruba
    ///
    /// ---
    /// AW 阿鲁巴 (荷兰王国)
    AW = 363,
    /// 🇦🇺 AU Australia
    ///
    /// ---
    /// AU 澳大利亚
    AU = 505,
    /// 🇦🇹 AT Austria
    ///
    /// ---
    /// AT 奥地利
    AT = 232,
    /// 🇦🇿 AZ Azerbaijan
    ///
    /// ---
    /// AZ 阿塞拜疆
    AZ = 400,
    /// 🇧🇸 BS Bahamas
    ///
    /// ---
    /// BS 巴哈马
    BS = 364,
    /// 🇧🇭 BH Bahrain
    ///
    /// ---
    /// BH 巴林
    BH = 426,
    /// 🇧🇩 BD Bangladesh
    ///
    /// ---
    /// BD 孟加拉国
    BD = 470,
    /// 🇧🇧 BB Barbados
    ///
    /// ---
    /// BB 巴巴多斯
    BB = 342,
    /// 🇧🇾 BY Belarus
    ///
    /// ---
    /// BY 白俄罗斯
    BY = 257,
    /// 🇧🇪 BE Belgium
    ///
    /// ---
    /// BE 比利时
    BE = 206,
    /// 🇧🇿 BZ Belize
    ///
    /// ---
    /// BZ 伯利兹
    BZ = 702,
    /// 🇧🇯 BJ Benin
    ///
    /// ---
    /// BJ 贝宁
    BJ = 616,
    /// 🇧🇲 BM Bermuda
    ///
    /// ---
    /// BM 百慕大 (英国)
    BM = 350,
    /// 🇧🇹 BT Bhutan
    ///
    /// ---
    /// BT 不丹
    BT = 402,
    /// 🇧🇴 BO Bolivia
    ///
    /// ---
    /// BO 玻利维亚
    BO = 736,
    /// 🇧🇦 BA Bosnia and Herzegovina
    ///
    /// ---
    /// BA 波斯尼亚和黑塞哥维那
    BA = 218,
    /// 🇧🇼 BW Botswana
    ///
    /// ---
    /// BW 博茨瓦纳
    BW = 652,
    /// 🇧🇷 BR Brazil
    ///
    /// ---
    /// BR 巴西
    BR = 724,
    /// 🇻🇬 VG British Virgin Islands
    ///
    /// ---
    /// VG 英属维尔京群岛 (英国)
    VG = 348,
    /// 🇧🇳 BN Brunei
    ///
    /// ---
    /// BN 文莱
    BN = 528,
    /// 🇧🇬 BG Bulgaria
    ///
    /// ---
    /// BG 保加利亚
    BG = 284,
    /// 🇧🇫 BF Burkina Faso
    ///
    /// ---
    /// BF 布基纳法索
    BF = 613,
    /// 🇧🇮 BI Burundi
    ///
    /// ---
    /// BI 布隆迪
    BI = 642,
    /// 🇰🇭 KH Cambodia
    ///
    /// ---
    /// KH 柬埔寨
    KH = 456,
    /// 🇨🇲 CM Cameroon
    ///
    /// ---
    /// CM 喀麦隆
    CM = 624,
    /// 🇨🇦 CA Canada
    ///
    /// ---
    /// CA 加拿大
    CA = 302,
    /// 🇨🇻 CV Cape Verde
    ///
    /// ---
    /// CV 佛得角
    CV = 625,
    /// 🇰🇾 KY Cayman Islands
    ///
    /// ---
    /// KY 开曼群岛 (英国)
    KY = 346,
    /// 🇨🇫 CF Central African Republic
    ///
    /// ---
    /// CF 中非共和国
    CF = 623,
    /// 🇹🇩 TD Chad
    ///
    /// ---
    /// TD 查德
    TD = 622,
    /// 🇨🇱 CL Chile
    ///
    /// ---
    /// CL 智利
    CL = 730,
    /// 🇨🇳 CN China
    ///
    /// ---
    /// CN 中国
    CN = 460,
    /// 🇨🇳 CN China
    ///
    /// ---
    /// CN 中国
    CN_1 = 461,
    /// 🇨🇴 CO Colombia
    ///
    /// ---
    /// CO 哥伦比亚
    CO = 732,
    /// 🇰🇲 KM Comoros
    ///
    /// ---
    /// KM 科摩罗
    KM = 654,
    /// 🇨🇬 CG Republic of the Congo
    ///
    /// ---
    /// CG 刚果共和国
    CG = 629,
    /// 🇨🇰 CK Cook Islands
    ///
    /// ---
    /// CK 库克群岛 (新西兰)
    CK = 548,
    /// 🇨🇷 CR Costa Rica
    ///
    /// ---
    /// CR 哥斯达黎加
    CR = 712,
    /// 🇨🇮 CI Côte d'Ivoire
    ///
    /// ---
    /// CI 科特迪瓦
    CI = 612,
    /// 🇭🇷 HR Croatia
    ///
    /// ---
    /// HR 克罗地亚
    HR = 219,
    /// 🇨🇺 CU Cuba
    ///
    /// ---
    /// CU 古巴
    CU = 368,
    /// 🇨🇼 CW Curaçao
    ///
    /// ---
    /// CW 库拉索 (荷兰王国)
    CW = 362,
    /// 🇨🇾 CY Cyprus
    ///
    /// ---
    /// CY 塞浦路斯
    CY = 280,
    /// 🇨🇿 CZ Czech Republic
    ///
    /// ---
    /// CZ 捷克
    CZ = 230,
    /// 🇨🇩 CD Democratic Republic of the Congo
    ///
    /// ---
    /// CD 刚果民主共和国
    CD = 630,
    /// 🇩🇰 DK Denmark
    ///
    /// ---
    /// DK 丹麦
    DK = 238,
    /// 🇩🇯 DJ Djibouti
    ///
    /// ---
    /// DJ 吉布提
    DJ = 638,
    /// 🇩🇲 DM Dominica
    ///
    /// ---
    /// DM 多米尼克
    DM = 366,
    /// 🇩🇴 DO Dominican Republic
    ///
    /// ---
    /// DO 多米尼加共和国
    DO = 370,
    /// 🇹🇱 TL East Timor
    ///
    /// ---
    /// TL 东帝汶
    TL = 514,
    /// 🇪🇨 EC Ecuador
    ///
    /// ---
    /// EC 厄瓜多尔
    EC = 740,
    /// 🇪🇬 EG Egypt
    ///
    /// ---
    /// EG 埃及
    EG = 602,
    /// 🇸🇻 SV El Salvador
    ///
    /// ---
    /// SV 萨尔瓦多
    SV = 706,
    /// 🇬🇶 GQ Equatorial Guinea
    ///
    /// ---
    /// GQ 赤道几内亚
    GQ = 627,
    /// 🇪🇷 ER Eritrea
    ///
    /// ---
    /// ER 厄立特里亚
    ER = 657,
    /// 🇪🇪 EE Estonia
    ///
    /// ---
    /// EE 爱沙尼亚
    EE = 248,
    /// 🇪🇹 ET Ethiopia
    ///
    /// ---
    /// ET 埃塞俄比亚
    ET = 636,
    /// 🇫🇰 FK Falkland Islands
    ///
    /// ---
    /// FK 福克兰群岛 (英国)
    FK = 750,
    /// 🇫🇴 FO Faroe Islands
    ///
    /// ---
    /// FO 法罗群岛 (丹麦)
    FO = 288,
    /// 🇫🇯 FJ Fiji
    ///
    /// ---
    /// FJ 斐济
    FJ = 542,
    /// 🇫🇮 FI Finland
    ///
    /// ---
    /// FI 芬兰
    FI = 244,
    /// 🇫🇷 FR France
    ///
    /// ---
    /// FR 法国
    FR = 208,
    /// 🇬🇫 GF French Guiana
    ///
    /// ---
    /// GF 法属圭亚那 (法国)
    GF = 742,
    /// 🇵🇫 PF French Polynesia
    ///
    /// ---
    /// PF 法属波利尼西亚 (法国)
    PF = 547,
    /// 🇬🇦 GA Gabon
    ///
    /// ---
    /// GA 加蓬
    GA = 628,
    /// 🇬🇲 GM Gambia
    ///
    /// ---
    /// GM 冈比亚
    GM = 607,
    /// 🇬🇪 GE Georgia
    ///
    /// ---
    /// GE 格鲁吉亚
    GE = 282,
    /// 🇩🇪 DE Germany
    ///
    /// ---
    /// DE 德国
    DE = 262,
    /// 🇬🇭 GH Ghana
    ///
    /// ---
    /// GH 加纳
    GH = 620,
    /// 🇬🇮 GI Gibraltar
    ///
    /// ---
    /// GI 直布罗陀 (英国)
    GI = 266,
    /// 🇬🇷 GR Greece
    ///
    /// ---
    /// GR 希腊
    GR = 202,
    /// 🇬🇱 GL Greenland
    ///
    /// ---
    /// GL 格陵兰 (丹麦)
    GL = 290,
    /// 🇬🇩 GD Grenada
    ///
    /// ---
    /// GD 格林纳达
    GD = 352,
    /// 🇬🇵 GP Guadeloupe
    /// 🇲🇶 MQ Martinique
    ///
    /// ---
    /// GP 瓜德罗普 (法国)
    /// MQ 马提尼克 (法国)
    GP_MQ = 340,
    /// 🇬🇺 GU Guam
    ///
    /// ---
    /// GU 关岛 (美国)
    GU = 535,
    /// 🇬🇹 GT Guatemala
    ///
    /// ---
    /// GT 危地马拉
    GT = 704,
    /// 🇬🇳 GN Guinea
    ///
    /// ---
    /// GN 几内亚
    GN = 611,
    /// 🇬🇼 GW Guinea-Bissau
    ///
    /// ---
    /// GW 几内亚比绍
    GW = 632,
    /// 🇬🇾 GY Guyana
    ///
    /// ---
    /// GY 圭亚那
    GY = 738,
    /// 🇭🇹 HT Haiti
    ///
    /// ---
    /// HT 海地
    HT = 372,
    /// 🇭🇳 HN Honduras
    ///
    /// ---
    /// HN 洪都拉斯
    HN = 708,
    /// 🇭🇰 HK Hong Kong
    ///
    /// ---
    /// HK 香港 (中国)
    HK = 454,
    /// 🇭🇺 HU Hungary
    ///
    /// ---
    /// HU 匈牙利
    HU = 216,
    /// 🇮🇸 IS Iceland
    ///
    /// ---
    /// IS 冰岛
    IS = 274,
    /// 🇮🇳 IN India
    ///
    /// ---
    /// IN 印度
    IN = 404,
    /// 🇮🇳 IN India
    ///
    /// ---
    /// IN 印度
    IN_1 = 405,
    /// 🇮🇳 IN India
    ///
    /// ---
    /// IN 印度
    IN_2 = 406,
    /// 🇮🇩 ID Indonesia
    ///
    /// ---
    /// ID 印度尼西亚
    ID = 510,
    /// 🇮🇷 IR Iran
    ///
    /// ---
    /// IR 伊朗
    IR = 432,
    /// 🇮🇶 IQ Iraq
    ///
    /// ---
    /// IQ 伊拉克
    IQ = 418,
    /// 🇮🇪 IE Ireland
    ///
    /// ---
    /// IE 爱尔兰共和国
    IE = 272,
    /// 🇵🇸 PS Palestine
    /// 🇮🇱 IL Israel
    ///
    /// ---
    /// IL 以色列巴勒斯坦
    IL_PS = 425,
    /// 🇮🇹 IT Italy
    ///
    /// ---
    /// IT 意大利
    IT = 222,
    /// 🇯🇲 JM Jamaica
    ///
    /// ---
    /// JM 牙买加
    JM = 338,
    /// 🇯🇵 JP Japan
    ///
    /// ---
    /// JP 日本
    JP = 441,
    /// 🇯🇵 JP Japan
    ///
    /// ---
    /// JP 日本
    JP_1 = 440,
    /// 🇯🇴 JO Jordan
    ///
    /// ---
    /// JO 约旦
    JO = 416,
    /// 🇰🇿 KZ Kazakhstan
    ///
    /// ---
    /// KZ 哈萨克斯坦
    KZ = 401,
    /// 🇰🇪 KE Kenya
    ///
    /// ---
    /// KE 肯尼亚
    KE = 639,
    /// 🇰🇮 KI Kiribati
    ///
    /// ---
    /// KI 基里巴斯
    KI = 545,
    /// 🇰🇵 KP North Korea
    ///
    /// ---
    /// KP 朝鲜
    KP = 467,
    /// 🇰🇷 KR South Korea
    ///
    /// ---
    /// KR 韩国
    KR = 450,
    /// 🇰🇼 KW Kuwait
    ///
    /// ---
    /// KW 科威特
    KW = 419,
    /// 🇰🇬 KG Kyrgyzstan
    ///
    /// ---
    /// KG 吉尔吉斯斯坦
    KG = 437,
    /// 🇱🇦 LA Laos
    ///
    /// ---
    /// LA 老挝
    LA = 457,
    /// 🇱🇻 LV Latvia
    ///
    /// ---
    /// LV 拉脱维亚
    LV = 247,
    /// 🇱🇧 LB Lebanon
    ///
    /// ---
    /// LB 黎巴嫩
    LB = 415,
    /// 🇱🇸 LS Lesotho
    ///
    /// ---
    /// LS 莱索托
    LS = 651,
    /// 🇱🇷 LR Liberia
    ///
    /// ---
    /// LR 利比里亚
    LR = 618,
    /// 🇱🇾 LY Libya
    ///
    /// ---
    /// LY 利比亚
    LY = 606,
    /// 🇱🇮 LI Liechtenstein
    ///
    /// ---
    /// LI 列支敦士登
    LI = 295,
    /// 🇱🇹 LT Lithuania
    ///
    /// ---
    /// LT 立陶宛
    LT = 246,
    /// 🇱🇺 LU Luxembourg
    ///
    /// ---
    /// LU 卢森堡
    LU = 270,
    /// 🇲🇴 MO Macau
    ///
    /// ---
    /// MO 澳门 (中国)
    MO = 455,
    /// 🇲🇰 MK North Macedonia
    ///
    /// ---
    /// MK 卢森堡
    MK = 294,
    /// 🇲🇬 MG Madagascar
    ///
    /// ---
    /// MG 马达加斯加
    MG = 646,
    /// 🇲🇼 MW Malawi
    ///
    /// ---
    /// MW 马拉维
    MW = 650,
    /// 🇲🇾 MY Malaysia
    ///
    /// ---
    /// MY 马来西亚
    MY = 502,
    /// 🇲🇻 MV Maldives
    ///
    /// ---
    /// MV 马尔代夫
    MV = 472,
    /// 🇲🇱 ML Mali
    ///
    /// ---
    /// ML 马里共和国
    ML = 610,
    /// 🇲🇹 MT Malta
    ///
    /// ---
    /// MT 马耳他
    MT = 278,
    /// 🇲🇭 MH Marshall Islands
    ///
    /// ---
    /// MH 马绍尔群岛
    MH = 551,
    /// 🇲🇷 MR Mauritania
    ///
    /// ---
    /// MR 毛里塔尼亚
    MR = 609,
    /// 🇲🇺 MU Mauritius
    ///
    /// ---
    /// MU 毛里求斯
    MU = 617,
    /// 🇲🇽 MX Mexico
    ///
    /// ---
    /// MX 墨西哥
    MX = 334,
    /// 🇫🇲 FM Micronesia
    ///
    /// ---
    /// FM 密克罗尼西亚联邦
    FM = 550,
    /// 🇲🇩 MD Moldova
    ///
    /// ---
    /// MD 摩尔多瓦
    MD = 259,
    /// 🇲🇨 MC Monaco
    ///
    /// ---
    /// MC 摩纳哥
    MC = 212,
    /// 🇲🇳 MN Mongolia
    ///
    /// ---
    /// MN 蒙古国
    MN = 428,
    /// 🇲🇪 ME Montenegro
    ///
    /// ---
    /// ME 黑山
    ME = 297,
    /// 🇲🇸 MS Montserrat
    ///
    /// ---
    /// MS 蒙塞拉特岛 (英国)
    MS = 354,
    /// 🇲🇦 MA Morocco
    ///
    /// ---
    /// MA 摩洛哥
    MA = 604,
    /// 🇲🇿 MZ Mozambique
    ///
    /// ---
    /// MZ 莫桑比克
    MZ = 643,
    /// 🇲🇲 MM Myanmar
    ///
    /// ---
    /// MM 缅甸
    MM = 414,
    /// 🇳🇦 NA Namibia
    ///
    /// ---
    /// NA 纳米比亚
    NA = 649,
    /// 🇳🇷 NR Nauru
    ///
    /// ---
    /// NR 瑙鲁
    NR = 536,
    /// 🇳🇵 NP Nepal
    ///
    /// ---
    /// NP 尼泊尔
    NP = 429,
    /// 🇳🇱 NL Netherlands
    ///
    /// ---
    /// NL 荷兰
    NL = 204,
    /// 🇳🇨 NC New Caledonia
    ///
    /// ---
    /// NC 新喀里多尼亚 (法国)
    NC = 546,
    /// 🇳🇿 NZ New Zealand
    ///
    /// ---
    /// NZ 新西兰
    NZ = 530,
    /// 🇳🇮 NI Nicaragua
    ///
    /// ---
    /// NI 尼加拉瓜
    NI = 710,
    /// 🇳🇪 NE Niger
    ///
    /// ---
    /// NE 尼日尔
    NE = 614,
    /// 🇳🇬 NG Nigeria
    ///
    /// ---
    /// NG 尼日利亚
    NG = 621,
    /// 🇳🇺 NU Niue
    ///
    /// ---
    /// NU 纽埃
    NU = 555,
    /// 🇲🇵 MP Northern Mariana Islands
    ///
    /// ---
    /// MP 北马里亚纳群岛 (美国)
    MP = 534,
    /// 🇳🇴 NO Norway
    ///
    /// ---
    /// NO 挪威
    NO = 242,
    /// 🇴🇲 OM Oman
    ///
    /// ---
    /// OM 阿曼
    OM = 422,
    /// 🇵🇰 PK Pakistan
    ///
    /// ---
    /// PK 巴基斯坦
    PK = 410,
    /// 🇵🇼 PW Palau
    ///
    /// ---
    /// PW 帕劳
    PW = 552,
    /// 🇵🇦 PA Panama
    ///
    /// ---
    /// PA 巴拿马
    PA = 714,
    /// 🇵🇬 PG Papua New Guinea
    ///
    /// ---
    /// PG 巴布亚新几内亚
    PG = 537,
    /// 🇵🇾 PY Paraguay
    ///
    /// ---
    /// PY 巴拉圭
    PY = 744,
    /// PE Peru
    ///
    /// ---
    /// PE 秘鲁
    PE = 716,
    /// PH Philippines
    ///
    /// ---
    /// PH 菲律宾
    PH = 515,
    /// PL Poland
    ///
    /// ---
    /// PL 波兰
    PL = 260,
    /// PT Portugal
    ///
    /// ---
    /// PT 葡萄牙
    PT = 268,
    /// PR Puerto Rico
    ///
    /// ---
    /// PR 波多黎各 (美国)
    PR = 330,
    /// QA Qatar
    ///
    /// ---
    /// QA 卡塔尔
    QA = 427,
    /// RE Réunion
    ///
    /// ---
    /// RE 留尼汪 (法国)
    RE = 647,
    /// RO Romania
    ///
    /// ---
    /// RO 罗马尼亚
    RO = 226,
    /// RU Russia
    ///
    /// ---
    /// RU 俄罗斯
    RU = 250,
    /// RW Rwanda
    ///
    /// ---
    /// RW 卢旺达
    RW = 635,
    /// 🇰🇳 KN Saint Kitts and Nevis
    ///
    /// ---
    /// KN 圣基茨和尼维斯
    KN = 356,
    /// 🇱🇨 LC Saint Lucia
    ///
    /// ---
    /// LC 圣卢西亚
    LC = 358,
    /// 🇵🇲 PM Saint Pierre and Miquelon
    ///
    /// ---
    /// PM 圣皮埃尔和密克隆群岛 (法国)
    PM = 308,
    /// 🇻🇨 VC Saint Vincent and the Grenadines
    ///
    /// ---
    /// VC 圣文森特和格林纳丁斯
    VC = 360,
    /// 🇼🇸 WS Samoa
    ///
    /// ---
    /// WS 萨摩亚
    WS = 549,
    /// 🇸🇲 SM San Marino
    ///
    /// ---
    /// SM 圣马力诺
    SM = 292,
    /// 🇸🇹 ST São Tomé and Príncipe
    ///
    /// ---
    /// ST 圣多美和普林西比
    ST = 626,
    /// 🇸🇦 SA Saudi Arabia
    ///
    /// ---
    /// SA 沙特阿拉伯
    SA = 420,
    /// 🇸🇳 SN Senegal
    ///
    /// ---
    /// SN 塞内加尔
    SN = 608,
    /// 🇷🇸 RS Serbia
    ///
    /// ---
    /// RS 塞尔维亚共和国
    RS = 220,
    /// 🇸🇨 SC Seychelles
    ///
    /// ---
    /// SC 塞舌尔
    SC = 633,
    /// 🇸🇱 SL Sierra Leone
    ///
    /// ---
    /// SL 塞拉利昂共和国
    SL = 619,
    /// 🇸🇬 SG Singapore
    ///
    /// ---
    /// SG 新加坡
    SG = 525,
    /// 🇸🇰 SK Slovakia
    ///
    /// ---
    /// SK 斯洛伐克
    SK = 231,
    /// 🇸🇮 SI Slovenia
    ///
    /// ---
    /// SI 斯洛文尼亚
    SI = 293,
    /// 🇸🇧 SB Solomon Islands
    ///
    /// ---
    /// SB 所罗门群岛
    SB = 540,
    /// 🇸🇴 SO Somalia
    ///
    /// ---
    /// SO 索马里
    SO = 637,
    /// 🇿🇦 ZA South Africa
    ///
    /// ---
    /// ZA 南非
    ZA = 655,
    /// 🇪🇸 ES Spain
    ///
    /// ---
    /// ES 西班牙
    ES = 214,
    /// 🇱🇰 LK Sri Lanka
    ///
    /// ---
    /// LK 斯里兰卡
    LK = 413,
    /// 🇸🇩 SD Sudan
    ///
    /// ---
    /// SD 苏丹
    SD = 634,
    /// 🇸🇷 SR Suriname
    ///
    /// ---
    /// SR 苏里南
    SR = 746,
    /// 🇸🇿 SZ Eswatini
    ///
    /// ---
    /// SZ 斯威士兰
    SZ = 653,
    /// 🇸🇪 SE Sweden
    ///
    /// ---
    /// SE 瑞典
    SE = 240,
    /// 🇨🇭 CH Switzerland
    ///
    /// ---
    /// CH 瑞士
    CH = 228,
    /// 🇸🇾 SY Syria
    ///
    /// ---
    /// SY 叙利亚
    SY = 417,
    /// 🇹🇼 TW Taiwan
    ///
    /// ---
    /// TW 台湾
    TW = 466,
    /// 🇹🇯 TJ Tajikistan
    ///
    /// ---
    /// TJ 塔吉克斯坦
    TJ = 436,
    /// 🇹🇿 TZ Tanzania
    ///
    /// ---
    /// TZ 坦桑尼亚
    TZ = 640,
    /// 🇹🇭 TH Thailand
    ///
    /// ---
    /// TH 泰国
    TH = 520,
    /// 🇹🇬 TG Togo
    ///
    /// ---
    /// TG 多哥
    TG = 615,
    /// 🇹🇴 TO Tonga
    ///
    /// ---
    /// TO 汤加
    TO = 539,
    /// 🇹🇹 TT Trinidad and Tobago
    ///
    /// ---
    /// TT 特立尼达和多巴哥
    TT = 374,
    /// 🇹🇳 TN Tunisia
    ///
    /// ---
    /// TN 突尼斯
    TN = 605,
    /// 🇹🇷 TR Turkey
    ///
    /// ---
    /// TR 土耳其
    TR = 286,
    /// 🇹🇲 TM Turkmenistan
    ///
    /// ---
    /// TM 土库曼斯坦
    TM = 438,
    /// 🇹🇨 TC Turks and Caicos Islands
    ///
    /// ---
    /// TC 特克斯和凯科斯群岛 (英国)
    TC = 376,
    /// 🇺🇬 UG Uganda
    ///
    /// ---
    /// UG 乌干达
    UG = 641,
    /// 🇺🇦 UA Ukraine
    ///
    /// ---
    /// UA 乌克兰
    UA = 255,
    /// 🇦🇪 AE United Arab Emirates
    ///
    /// ---
    /// AE 阿拉伯联合酋长国
    AE = 424,
    /// 🇦🇪 AE United Arab Emirates
    ///
    /// ---
    /// AE 阿拉伯联合酋长国
    AE_1 = 430,
    /// 🇦🇪 AE United Arab Emirates
    ///
    /// ---
    /// AE 阿拉伯联合酋长国
    AE_2 = 431,
    /// 🇬🇧 GB United Kingdom
    ///
    /// ---
    /// GB 英国
    GB = 235,
    /// 🇬🇧 GB United Kingdom
    ///
    /// ---
    /// GB 英国
    GB_1 = 234,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US = 310,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_1 = 311,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_2 = 312,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_3 = 313,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_4 = 314,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_5 = 315,
    /// 🇺🇸 US United States
    ///
    /// ---
    /// US 美国
    US_6 = 316,
    /// 🇻🇮 VI United States Virgin Islands
    ///
    /// ---
    /// VI 美属维尔京群岛 (美国)
    VI = 332,
    /// 🇺🇾 UY Uruguay
    ///
    /// ---
    /// UY 乌拉圭
    UY = 748,
    /// 🇺🇿 UZ Uzbekistan
    ///
    /// ---
    /// UZ 乌兹别克斯坦
    UZ = 434,
    /// 🇻🇺 VU Vanuatu
    ///
    /// ---
    /// VU 瓦努阿图
    VU = 541,
    /// 🇻🇦 VA Vatican City
    ///
    /// ---
    /// VA 梵蒂冈
    VA = 225,
    /// 🇻🇪 VE Venezuela
    ///
    /// ---
    /// VE 委内瑞拉
    VE = 734,
    /// 🇻🇳 VN Vietnam
    ///
    /// ---
    /// VN 越南
    VN = 452,
    /// 🇼🇫 WF Wallis and Futuna
    ///
    /// ---
    /// WF 瓦利斯和富图纳群岛 (法国)
    WF = 543,
    /// 🇾🇪 YE Yemen
    ///
    /// ---
    /// YE 也门
    YE = 421,
    /// 🇿🇲 ZM Zambia
    ///
    /// ---
    /// ZM 赞比亚
    ZM = 645,
    /// 🇿🇼 ZW Zimbabwe
    ///
    /// ---
    /// ZW 津巴布韦
    ZW = 648,
}

impl MCC {
    /// Check if the mcc is a valid MCC code with value.
    pub fn is<T>(mcc: T) -> bool
    where
        T: PartialEq<u32> + Copy,
    {
        MCC::iter().any(|variant| mcc == variant as u32)
    }

    /// Check if the mcc is a valid MCC code with string `mcc`.
    pub fn is_code(mcc: String) -> bool {
        if !mcc.starts_with("mcc") {
            return false;
        }
        let mcc = mcc.replace("mcc", "");
        match mcc.parse::<u32>() {
            Ok(mcc) => MCC::is(mcc),
            Err(_) => false,
        }
    }
}
