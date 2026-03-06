use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/**
 * Country/region code
 * - Follows the ISO 3166-1 standard, usually 2 uppercase letters.
 * - Example: 'CN' (China), 'US' (United States), 'GB' (United Kingdom).
 * - For a detailed list, please refer to: https://www.iso.org/iso-3166-country-codes.html
 *
 * ---
 *
 * 国家/地区代码 (Region Code)
 * - 遵循 ISO 3166-1 标准，通常为2个大写字母。
 * - 例如: 'CN' (中国), 'US' (美国), 'GB' (英国)。
 * - 详细列表请查阅: https://www.iso.org/iso-3166-country-codes.html
 */
#[derive(EnumIter, strum_macros::Display)]
pub enum RegionCode {
    /**
     * 🇦🇫 Afghanistan
     *
     * ---
     *
     * 阿富汗
     */
    AF,
    /**
     * 🇦🇽 Åland Islands
     *
     * ---
     *
     * 奥兰群岛
     */
    AX,
    /**
     * 🇦🇱 Albania
     *
     * ---
     *
     * 阿尔巴尼亚
     */
    AL,
    /**
     * 🇩🇿 Algeria
     *
     * ---
     *
     * 阿尔及利亚
     */
    DZ,
    /**
     * 🇦🇸 American Samoa
     *
     * ---
     *
     * 美属萨摩亚
     */
    AS,
    /**
     * 🇦🇩 Andorra
     *
     * ---
     *
     * 安道尔
     */
    AD,
    /**
     * 🇦🇴 Angola
     *
     * ---
     *
     * 安哥拉
     */
    AO,
    /**
     * 🇦🇮 Anguilla
     *
     * ---
     *
     * 安圭拉
     */
    AI,
    /**
     * 🇦🇶 Antarctica
     *
     * ---
     *
     * 南极洲
     */
    AQ,
    /**
     * 🇦🇬 Antigua and Barbuda
     *
     * ---
     *
     * 安提瓜和巴布达
     */
    AG,
    /**
     * 🇦🇷 Argentina
     *
     * ---
     *
     * 阿根廷
     */
    AR,
    /**
     * 🇦🇲 Armenia
     *
     * ---
     *
     * 亚美尼亚
     */
    AM,
    /**
     * 🇦🇼 Aruba
     *
     * ---
     *
     * 阿鲁巴
     */
    AW,
    /**
     * 🇦🇺 Australia
     *
     * ---
     *
     * 澳大利亚
     */
    AU,
    /**
     * 🇦🇹 Austria
     *
     * ---
     *
     * 奥地利
     */
    AT,
    /**
     * 🇦🇿 Azerbaijan
     *
     * ---
     *
     * 阿塞拜疆
     */
    AZ,
    /**
     * 🇧🇸 Bahamas
     *
     * ---
     *
     * 巴哈马
     */
    BS,
    /**
     * 🇧🇭 Bahrain
     *
     * ---
     *
     * 巴林
     */
    BH,
    /**
     * 🇧🇩 Bangladesh
     *
     * ---
     *
     * 孟加拉国
     */
    BD,
    /**
     * 🇧🇧 Barbados
     *
     * ---
     *
     * 巴巴多斯
     */
    BB,
    /**
     * 🇧🇾 Belarus
     *
     * ---
     *
     * 白俄罗斯
     */
    BY,
    /**
     * 🇧🇪 Belgium
     *
     * ---
     *
     * 比利时
     */
    BE,
    /**
     * 🇧🇿 Belize
     *
     * ---
     *
     * 伯利兹
     */
    BZ,
    /**
     * 🇧🇯 Benin
     *
     * ---
     *
     * 贝宁
     */
    BJ,
    /**
     * 🇧🇲 Bermuda
     *
     * ---
     *
     * 百慕大
     */
    BM,
    /**
     * 🇧🇹 Bhutan
     *
     * ---
     *
     * 不丹
     */
    BT,
    /**
     * 🇧🇴 Bolivia (Plurinational State of)
     *
     * ---
     *
     * 玻利维亚（多民族国）
     */
    BO,
    /**
     * 🇧🇶 Bonaire, Sint Eustatius and Saba
     *
     * ---
     *
     * 博奈尔、圣尤斯特歇斯和萨巴
     */
    BQ,
    /**
     * 🇧🇦 Bosnia and Herzegovina
     *
     * ---
     *
     * 波斯尼亚和黑塞哥维那
     */
    BA,
    /**
     * 🇧🇼 Botswana
     *
     * ---
     *
     * 博茨瓦纳
     */
    BW,
    /**
     * 🇧🇻 Bouvet Island
     *
     * ---
     *
     * 布韦岛
     */
    BV,
    /**
     * 🇧🇷 Brazil
     *
     * ---
     *
     * 巴西
     */
    BR,
    /**
     * 🇮🇴 British Indian Ocean Territory
     *
     * ---
     *
     * 英属印度洋领地
     */
    IO,
    /**
     * 🇧🇳 Brunei Darussalam
     *
     * ---
     *
     * 文莱达鲁萨兰国
     */
    BN,
    /**
     * 🇧🇬 Bulgaria
     *
     * ---
     *
     * 保加利亚
     */
    BG,
    /**
     * 🇧🇫 Burkina Faso
     *
     * ---
     *
     * 布基纳法索
     */
    BF,
    /**
     * 🇧🇮 Burundi
     *
     * ---
     *
     * 布隆迪
     */
    BI,
    /**
     * 🇨🇻 Cabo Verde
     *
     * ---
     *
     * 佛得角
     */
    CV,
    /**
     * 🇰🇭 Cambodia
     *
     * ---
     *
     * 柬埔寨
     */
    KH,
    /**
     * 🇨🇲 Cameroon
     *
     * ---
     *
     * 喀麦隆
     */
    CM,
    /**
     * 🇨🇦 Canada
     *
     * ---
     *
     * 加拿大
     */
    CA,
    /**
     * 🇰🇾 Cayman Islands
     *
     * ---
     *
     * 开曼群岛
     */
    KY,
    /**
     * 🇨🇫 Central African Republic
     *
     * ---
     *
     * 中非共和国
     */
    CF,
    /**
     * 🇹🇩 Chad
     *
     * ---
     *
     * 乍得
     */
    TD,
    /**
     * 🇨🇱 Chile
     *
     * ---
     *
     * 智利
     */
    CL,
    /**
     * 🇨🇳 China
     *
     * ---
     *
     * 中国
     */
    CN,
    /**
     * 🇨🇽 Christmas Island
     *
     * ---
     *
     * 圣诞岛
     */
    CX,
    /**
     * 🇨🇨 Cocos (Keeling) Islands
     *
     * ---
     *
     * 科科斯（基林）群岛
     */
    CC,
    /**
     * 🇨🇴 Colombia
     *
     * ---
     *
     * 哥伦比亚
     */
    CO,
    /**
     * 🇰🇲 Comoros
     *
     * ---
     *
     * 科摩罗
     */
    KM,
    /**
     * 🇨🇬 Congo
     *
     * ---
     *
     * 刚果
     */
    CG,
    /**
     * 🇨🇩 Congo (Democratic Republic of the)
     *
     * ---
     *
     * 刚果（民主共和国）
     */
    CD,
    /**
     * 🇨🇰 Cook Islands
     *
     * ---
     *
     * 库克群岛
     */
    CK,
    /**
     * 🇨🇷 Costa Rica
     *
     * ---
     *
     * 哥斯达黎加
     */
    CR,
    /**
     * 🇨🇮 Côte d'Ivoire
     *
     * ---
     *
     * 科特迪瓦
     */
    CI,
    /**
     * 🇭🇷 Croatia
     *
     * ---
     *
     * 克罗地亚
     */
    HR,
    /**
     * 🇨🇺 Cuba
     *
     * ---
     *
     * 古巴
     */
    CU,
    /**
     * 🇨🇼 Curaçao
     *
     * ---
     *
     * 库拉索
     */
    CW,
    /**
     * 🇨🇾 Cyprus
     *
     * ---
     *
     * 塞浦路斯
     */
    CY,
    /**
     * 🇨🇿 Czechia
     *
     * ---
     *
     * 捷克
     */
    CZ,
    /**
     * 🇩🇰 Denmark
     *
     * ---
     *
     * 丹麦
     */
    DK,
    /**
     * 🇩🇯 Djibouti
     *
     * ---
     *
     * 吉布提
     */
    DJ,
    /**
     * 🇩🇲 Dominica
     *
     * ---
     *
     * 多米尼克
     */
    DM,
    /**
     * 🇩🇴 Dominican Republic
     *
     * ---
     *
     * 多米尼加共和国
     */
    DO,
    /**
     * 🇪🇨 Ecuador
     *
     * ---
     *
     * 厄瓜多尔
     */
    EC,
    /**
     * 🇪🇬 Egypt
     *
     * ---
     *
     * 埃及
     */
    EG,
    /**
     * 🇸🇻 El Salvador
     *
     * ---
     *
     * 萨尔瓦多
     */
    SV,
    /**
     * 🇬🇶 Equatorial Guinea
     *
     * ---
     *
     * 赤道几内亚
     */
    GQ,
    /**
     * 🇪🇷 Eritrea
     *
     * ---
     *
     * 厄立特里亚
     */
    ER,
    /**
     * 🇪🇪 Estonia
     *
     * ---
     *
     * 爱沙尼亚
     */
    EE,
    /**
     * 🇸🇿 Eswatini
     *
     * ---
     *
     * 斯威士兰
     */
    SZ,
    /**
     * 🇪🇹 Ethiopia
     *
     * ---
     *
     * 埃塞俄比亚
     */
    ET,
    /**
     * 🇫🇰 Falkland Islands (Malvinas)
     *
     * ---
     *
     * 福克兰群岛（马尔维纳斯）
     */
    FK,
    /**
     * 🇫🇴 Faroe Islands
     *
     * ---
     *
     * 法罗群岛
     */
    FO,
    /**
     * 🇫🇯 Fiji
     *
     * ---
     *
     * 斐济
     */
    FJ,
    /**
     * 🇫🇮 Finland
     *
     * ---
     *
     * 芬兰
     */
    FI,
    /**
     * 🇫🇷 France
     *
     * ---
     *
     * 法国
     */
    FR,
    /**
     * 🇬🇫 French Guiana
     *
     * ---
     *
     * 法属圭亚那
     */
    GF,
    /**
     * 🇵🇫 French Polynesia
     *
     * ---
     *
     * 法属波利尼西亚
     */
    PF,
    /**
     * 🇹🇫 French Southern Territories
     *
     * ---
     *
     * 法属南部领地
     */
    TF,
    /**
     * 🇬🇦 Gabon
     *
     * ---
     *
     * 加蓬
     */
    GA,
    /**
     * 🇬🇲 Gambia
     *
     * ---
     *
     * 冈比亚
     */
    GM,
    /**
     * 🇬🇪 Georgia
     *
     * ---
     *
     * 格鲁吉亚
     */
    GE,
    /**
     * 🇩🇪 Germany
     *
     * ---
     *
     * 德国
     */
    DE,
    /**
     * 🇬🇭 Ghana
     *
     * ---
     *
     * 加纳
     */
    GH,
    /**
     * 🇬🇮 Gibraltar
     *
     * ---
     *
     * 直布罗陀
     */
    GI,
    /**
     * 🇬🇷 Greece
     *
     * ---
     *
     * 希腊
     */
    GR,
    /**
     * 🇬🇱 Greenland
     *
     * ---
     *
     * 格陵兰
     */
    GL,
    /**
     * 🇬🇩 Grenada
     *
     * ---
     *
     * 格林纳达
     */
    GD,
    /**
     * 🇬🇵 Guadeloupe
     *
     * ---
     *
     * 瓜德罗普
     */
    GP,
    /**
     * 🇬🇺 Guam
     *
     * ---
     *
     * 关岛
     */
    GU,
    /**
     * 🇬🇹 Guatemala
     *
     * ---
     *
     * 危地马拉
     */
    GT,
    /**
     * 🇬🇬 Guernsey
     *
     * ---
     *
     * 根西
     */
    GG,
    /**
     * 🇬🇳 Guinea
     *
     * ---
     *
     * 几内亚
     */
    GN,
    /**
     * 🇬🇼 Guinea-Bissau
     *
     * ---
     *
     * 几内亚比绍
     */
    GW,
    /**
     * 🇬🇾 Guyana
     *
     * ---
     *
     * 圭亚那
     */
    GY,
    /**
     * 🇭🇹 Haiti
     *
     * ---
     *
     * 海地
     */
    HT,
    /**
     * 🇭🇲 Heard Island and McDonald Islands
     *
     * ---
     *
     * 赫德岛和麦克唐纳群岛
     */
    HM,
    /**
     * 🇻🇦 Holy See
     *
     * ---
     *
     * 梵蒂冈
     */
    VA,
    /**
     * 🇭🇳 Honduras
     *
     * ---
     *
     * 洪都拉斯
     */
    HN,
    /**
     * 🇭🇰 Hong Kong
     *
     * ---
     *
     * 香港
     */
    HK,
    /**
     * 🇭🇺 Hungary
     *
     * ---
     *
     * 匈牙利
     */
    HU,
    /**
     * 🇮🇸 Iceland
     *
     * ---
     *
     * 冰岛
     */
    IS,
    /**
     * 🇮🇳 India
     *
     * ---
     *
     * 印度
     */
    IN,
    /**
     * 🇮🇩 Indonesia
     *
     * ---
     *
     * 印度尼西亚
     */
    ID,
    /**
     * 🇮🇷 Iran (Islamic Republic of)
     *
     * ---
     *
     * 伊朗（伊斯兰共和国）
     */
    IR,
    /**
     * 🇮🇶 Iraq
     *
     * ---
     *
     * 伊拉克
     */
    IQ,
    /**
     * 🇮🇪 Ireland
     *
     * ---
     *
     * 爱尔兰
     */
    IE,
    /**
     * 🇮🇲 Isle of Man
     *
     * ---
     *
     * 马恩岛
     */
    IM,
    /**
     * 🇮🇱 Israel
     *
     * ---
     *
     * 以色列
     */
    IL,
    /**
     * 🇮🇹 Italy
     *
     * ---
     *
     * 意大利
     */
    IT,
    /**
     * 🇯🇲 Jamaica
     *
     * ---
     *
     * 牙买加
     */
    JM,
    /**
     * 🇯🇵 Japan
     *
     * ---
     *
     * 日本
     */
    JP,
    /**
     * 🇯🇪 Jersey
     *
     * ---
     *
     * 泽西
     */
    JE,
    /**
     * 🇯🇴 Jordan
     *
     * ---
     *
     * 约旦
     */
    JO,
    /**
     * 🇰🇿 Kazakhstan
     *
     * ---
     *
     * 哈萨克斯坦
     */
    KZ,
    /**
     * 🇰🇪 Kenya
     *
     * ---
     *
     * 肯尼亚
     */
    KE,
    /**
     * 🇰🇮 Kiribati
     *
     * ---
     *
     * 基里巴斯
     */
    KI,
    /**
     * 🇰🇵 Korea (Democratic People's Republic of)
     *
     * ---
     *
     * 朝鲜（民主主义人民共和国）
     */
    KP,
    /**
     * 🇰🇷 Korea (Republic of)
     *
     * ---
     *
     * 韩国
     */
    KR,
    /**
     * 🇰🇼 Kuwait
     *
     * ---
     *
     * 科威特
     */
    KW,
    /**
     * 🇰🇬 Kyrgyzstan
     *
     * ---
     *
     * 吉尔吉斯斯坦
     */
    KG,
    /**
     * 🇱🇦 Lao People's Democratic Republic
     *
     * ---
     *
     * 老挝人民民主共和国
     */
    LA,
    /**
     * 🇱🇻 Latvia
     *
     * ---
     *
     * 拉脱维亚
     */
    LV,
    /**
     * 🇱🇧 Lebanon
     *
     * ---
     *
     * 黎巴嫩
     */
    LB,
    /**
     * 🇱🇸 Lesotho
     *
     * ---
     *
     * 莱索托
     */
    LS,
    /**
     * 🇱🇷 Liberia
     *
     * ---
     *
     * 利比里亚
     */
    LR,
    /**
     * 🇱🇾 Libya
     *
     * ---
     *
     * 利比亚
     */
    LY,
    /**
     * 🇱🇮 Liechtenstein
     *
     * ---
     *
     * 列支敦士登
     */
    LI,
    /**
     * 🇱🇹 Lithuania
     *
     * ---
     *
     * 立陶宛
     */
    LT,
    /**
     * 🇱🇺 Luxembourg
     *
     * ---
     *
     * 卢森堡
     */
    LU,
    /**
     * 🇲🇴 Macao
     *
     * ---
     *
     * 澳门
     */
    MO,
    /**
     * 🇲🇬 Madagascar
     *
     * ---
     *
     * 马达加斯加
     */
    MG,
    /**
     * 🇲🇼 Malawi
     *
     * ---
     *
     * 马拉维
     */
    MW,
    /**
     * 🇲🇾 Malaysia
     *
     * ---
     *
     * 马来西亚
     */
    MY,
    /**
     * 🇲🇻 Maldives
     *
     * ---
     *
     * 马尔代夫
     */
    MV,
    /**
     * 🇲🇱 Mali
     *
     * ---
     *
     * 马里
     */
    ML,
    /**
     * 🇲🇹 Malta
     *
     * ---
     *
     * 马耳他
     */
    MT,
    /**
     * 🇲🇭 Marshall Islands
     *
     * ---
     *
     * 马绍尔群岛
     */
    MH,
    /**
     * 🇲🇶 Martinique
     *
     * ---
     *
     * 马提尼克
     */
    MQ,
    /**
     * 🇲🇷 Mauritania
     *
     * ---
     *
     * 毛里塔尼亚
     */
    MR,
    /**
     * 🇲🇺 Mauritius
     *
     * ---
     *
     * 毛里求斯
     */
    MU,
    /**
     * 🇾🇹 Mayotte
     *
     * ---
     *
     * 马约特
     */
    YT,
    /**
     * 🇲🇽 Mexico
     *
     * ---
     *
     * 墨西哥
     */
    MX,
    /**
     * 🇫🇲 Micronesia (Federated States of)
     *
     * ---
     *
     * 密克罗尼西亚（联邦）
     */
    FM,
    /**
     * 🇲🇩 Moldova (Republic of)
     *
     * ---
     *
     * 摩尔多瓦（共和国）
     */
    MD,
    /**
     * 🇲🇨 Monaco
     *
     * ---
     *
     * 摩纳哥
     */
    MC,
    /**
     * 🇲🇳 Mongolia
     *
     * ---
     *
     * 蒙古
     */
    MN,
    /**
     * 🇲🇪 Montenegro
     *
     * ---
     *
     * 黑山
     */
    ME,
    /**
     * 🇲🇸 Montserrat
     *
     * ---
     *
     * 蒙特塞拉特
     */
    MS,
    /**
     * 🇲🇦 Morocco
     *
     * ---
     *
     * 摩洛哥
     */
    MA,
    /**
     * 🇲🇿 Mozambique
     *
     * ---
     *
     * 莫桑比克
     */
    MZ,
    /**
     * 🇲🇲 Myanmar
     *
     * ---
     *
     * 缅甸
     */
    MM,
    /**
     * 🇳🇦 Namibia
     *
     * ---
     *
     * 纳米比亚
     */
    NA,
    /**
     * 🇳🇷 Nauru
     *
     * ---
     *
     * 瑙鲁
     */
    NR,
    /**
     * 🇳🇵 Nepal
     *
     * ---
     *
     * 尼泊尔
     */
    NP,
    /**
     * 🇳🇱 Netherlands
     *
     * ---
     *
     * 荷兰
     */
    NL,
    /**
     * 🇳🇨 New Caledonia
     *
     * ---
     *
     * 新喀里多尼亚
     */
    NC,
    /**
     * 🇳🇿 New Zealand
     *
     * ---
     *
     * 新西兰
     */
    NZ,
    /**
     * 🇳🇮 Nicaragua
     *
     * ---
     *
     * 尼加拉瓜
     */
    NI,
    /**
     * 🇳🇪 Niger
     *
     * ---
     *
     * 尼日尔
     */
    NE,
    /**
     * 🇳🇬 Nigeria
     *
     * ---
     *
     * 尼日利亚
     */
    NG,
    /**
     * 🇳🇺 Niue
     *
     * ---
     *
     * 纽埃
     */
    NU,
    /**
     * 🇳🇫 Norfolk Island
     *
     * ---
     *
     * 诺福克岛
     */
    NF,
    /**
     * 🇲🇰 North Macedonia
     *
     * ---
     *
     * 北马其顿
     */
    MK,
    /**
     * 🇲🇵 Northern Mariana Islands
     *
     * ---
     *
     * 北马里亚纳群岛
     */
    MP,
    /**
     * 🇳🇴 Norway
     *
     * ---
     *
     * 挪威
     */
    NO,
    /**
     * 🇴🇲 Oman
     *
     * ---
     *
     * 阿曼
     */
    OM,
    /**
     * 🇵🇰 Pakistan
     *
     * ---
     *
     * 巴基斯坦
     */
    PK,
    /**
     * 🇵🇼 Palau
     *
     * ---
     *
     * 帕劳
     */
    PW,
    /**
     * 🇵🇸 Palestine, State of
     *
     * ---
     *
     * 巴勒斯坦国
     */
    PS,
    /**
     * 🇵🇦 Panama
     *
     * ---
     *
     * 巴拿马
     */
    PA,
    /**
     * 🇵🇬 Papua New Guinea
     *
     * ---
     *
     * 巴布亚新几内亚
     */
    PG,
    /**
     * 🇵🇾 Paraguay
     *
     * ---
     *
     * 巴拉圭
     */
    PY,
    /**
     * 🇵🇪 Peru
     *
     * ---
     *
     * 秘鲁
     */
    PE,
    /**
     * 🇵🇭 Philippines
     *
     * ---
     *
     * 菲律宾
     */
    PH,
    /**
     * 🇵🇳 Pitcairn
     *
     * ---
     *
     * 皮特凯恩
     */
    PN,
    /**
     * 🇵🇱 Poland
     *
     * ---
     *
     * 波兰
     */
    PL,
    /**
     * 🇵🇹 Portugal
     *
     * ---
     *
     * 葡萄牙
     */
    PT,
    /**
     * 🇵🇷 Puerto Rico
     *
     * ---
     *
     * 波多黎各
     */
    PR,
    /**
     * 🇶🇦 Qatar
     *
     * ---
     *
     * 卡塔尔
     */
    QA,
    /**
     * 🇷🇪 Réunion
     *
     * ---
     *
     * 留尼汪
     */
    RE,
    /**
     * 🇷🇴 Romania
     *
     * ---
     *
     * 罗马尼亚
     */
    RO,
    /**
     * 🇷🇺 Russian Federation
     *
     * ---
     *
     * 俄罗斯联邦
     */
    RU,
    /**
     * 🇷🇼 Rwanda
     *
     * ---
     *
     * 卢旺达
     */
    RW,
    /**
     * 🇧🇱 Saint Barthélemy
     *
     * ---
     *
     * 圣巴泰勒米
     */
    BL,
    /**
     * 🇸🇭 Saint Helena, Ascension and Tristan da Cunha
     *
     * ---
     *
     * 圣赫勒拿、阿森松和特里斯坦-达库尼亚
     */
    SH,
    /**
     * 🇰🇳 Saint Kitts and Nevis
     *
     * ---
     *
     * 圣基茨和尼维斯
     */
    KN,
    /**
     * 🇱🇨 Saint Lucia
     *
     * ---
     *
     * 圣卢西亚
     */
    LC,
    /**
     * 🇲🇫 Saint Martin (French part)
     *
     * ---
     *
     * 圣马丁（法属部分）
     */
    MF,
    /**
     * 🇵🇲 Saint Pierre and Miquelon
     *
     * ---
     *
     * 圣皮埃尔和密克隆
     */
    PM,
    /**
     * 🇻🇨 Saint Vincent and the Grenadines
     *
     * ---
     *
     * 圣文森特和格林纳丁斯
     */
    VC,
    /**
     * 🇼🇸 Samoa
     *
     * ---
     *
     * 萨摩亚
     */
    WS,
    /**
     * 🇸🇲 San Marino
     *
     * ---
     *
     * 圣马力诺
     */
    SM,
    /**
     * 🇸🇹 Sao Tome and Principe
     *
     * ---
     *
     * 圣多美和普林西比
     */
    ST,
    /**
     * 🇸🇦 Saudi Arabia
     *
     * ---
     *
     * 沙特阿拉伯
     */
    SA,
    /**
     * 🇸🇳 Senegal
     *
     * ---
     *
     * 塞内加尔
     */
    SN,
    /**
     * 🇷🇸 Serbia
     *
     * ---
     *
     * 塞尔维亚
     */
    RS,
    /**
     * 🇸🇨 Seychelles
     *
     * ---
     *
     * 塞舌尔
     */
    SC,
    /**
     * 🇸🇱 Sierra Leone
     *
     * ---
     *
     * 塞拉利昂
     */
    SL,
    /**
     * 🇸🇬 Singapore
     *
     * ---
     *
     * 新加坡
     */
    SG,
    /**
     * 🇸🇽 Sint Maarten (Dutch part)
     *
     * ---
     *
     * 圣马丁（荷属部分）
     */
    SX,
    /**
     * 🇸🇰 Slovakia
     *
     * ---
     *
     * 斯洛伐克
     */
    SK,
    /**
     * 🇸🇮 Slovenia
     *
     * ---
     *
     * 斯洛文尼亚
     */
    SI,
    /**
     * 🇸🇧 Solomon Islands
     *
     * ---
     *
     * 所罗门群岛
     */
    SB,
    /**
     * 🇸🇴 Somalia
     *
     * ---
     *
     * 索马里
     */
    SO,
    /**
     * 🇿🇦 South Africa
     *
     * ---
     *
     * 南非
     */
    ZA,
    /**
     * 🇬🇸 South Georgia and the South Sandwich Islands
     *
     * ---
     *
     * 南乔治亚岛和南桑威奇群岛
     */
    GS,
    /**
     * 🇸🇸 South Sudan
     *
     * ---
     *
     * 南苏丹
     */
    SS,
    /**
     * 🇪🇸 Spain
     *
     * ---
     *
     * 西班牙
     */
    ES,
    /**
     * 🇱🇰 Sri Lanka
     *
     * ---
     *
     * 斯里兰卡
     */
    LK,
    /**
     * 🇸🇩 Sudan
     *
     * ---
     *
     * 苏丹
     */
    SD,
    /**
     * 🇸🇷 Suriname
     *
     * ---
     *
     * 苏里南
     */
    SR,
    /**
     * 🇸🇯 Svalbard and Jan Mayen
     *
     * ---
     *
     * 斯瓦尔巴和扬马延
     */
    SJ,
    /**
     * 🇸🇪 Sweden
     *
     * ---
     *
     * 瑞典
     */
    SE,
    /**
     * 🇨🇭 Switzerland
     *
     * ---
     *
     * 瑞士
     */
    CH,
    /**
     * 🇸🇾 Syrian Arab Republic
     *
     * ---
     *
     * 阿拉伯叙利亚共和国
     */
    SY,
    /**
     * 🇹🇼 Taiwan, Province of China
     *
     * ---
     *
     * 中国台湾省
     */
    TW,
    /**
     * 🇹🇯 Tajikistan
     *
     * ---
     *
     * 塔吉克斯坦
     */
    TJ,
    /**
     * 🇹🇿 Tanzania, United Republic of
     *
     * ---
     *
     * 坦桑尼亚联合共和国
     */
    TZ,
    /**
     * 🇹🇭 Thailand
     *
     * ---
     *
     * 泰国
     */
    TH,
    /**
     * 🇹🇱 Timor-Leste
     *
     * ---
     *
     * 东帝汶
     */
    TL,
    /**
     * 🇹🇬 Togo
     *
     * ---
     *
     * 多哥
     */
    TG,
    /**
     * 🇹🇰 Tokelau
     *
     * ---
     *
     * 托克劳
     */
    TK,
    /**
     * 🇹🇴 Tonga
     *
     * ---
     *
     * 汤加
     */
    TO,
    /**
     * 🇹🇹 Trinidad and Tobago
     *
     * ---
     *
     * 特立尼达和多巴哥
     */
    TT,
    /**
     * 🇹🇳 Tunisia
     *
     * ---
     *
     * 突尼斯
     */
    TN,
    /**
     * 🇹🇷 Türkiye
     *
     * ---
     *
     * 土耳其
     */
    TR,
    /**
     * 🇹🇲 Turkmenistan
     *
     * ---
     *
     * 土库曼斯坦
     */
    TM,
    /**
     * 🇹🇨 Turks and Caicos Islands
     *
     * ---
     *
     * 特克斯和凯科斯群岛
     */
    TC,
    /**
     * 🇹🇻 Tuvalu
     *
     * ---
     *
     * 图瓦卢
     */
    TV,
    /**
     * 🇺🇬 Uganda
     *
     * ---
     *
     * 乌干达
     */
    UG,
    /**
     * 🇺🇦 Ukraine
     *
     * ---
     *
     * 乌克兰
     */
    UA,
    /**
     * 🇦🇪 United Arab Emirates
     *
     * ---
     *
     * 阿拉伯联合酋长国
     */
    AE,
    /**
     * 🇬🇧 United Kingdom of Great Britain and Northern Ireland
     *
     * ---
     *
     * 大不列颠及北爱尔兰联合王国
     */
    GB,
    /**
     * 🇺🇸 United States of America
     *
     * ---
     *
     * 美利坚合众国
     */
    US,
    /**
     * 🇺🇲 United States Minor Outlying Islands
     *
     * ---
     *
     * 美国本土外小岛屿
     */
    UM,
    /**
     * 🇺🇾 Uruguay
     *
     * ---
     *
     * 乌拉圭
     */
    UY,
    /**
     * 🇺🇿 Uzbekistan
     *
     * ---
     *
     * 乌兹别克斯坦
     */
    UZ,
    /**
     * 🇻🇺 Vanuatu
     *
     * ---
     *
     * 瓦努阿图
     */
    VU,
    /**
     * 🇻🇪 Venezuela (Bolivarian Republic of)
     *
     * ---
     *
     * 委内瑞拉（玻利瓦尔共和国）
     */
    VE,
    /**
     * 🇻🇳 Viet Nam
     *
     * ---
     *
     * 越南
     */
    VN,
    /**
     * 🇻🇬 Virgin Islands (British)
     *
     * ---
     *
     * 英属维尔京群岛
     */
    VG,
    /**
     * 🇻🇮 Virgin Islands (U.S.)
     *
     * ---
     *
     * 美属维尔京群岛
     */
    VI,
    /**
     * 🇼🇫 Wallis and Futuna
     *
     * ---
     *
     * 瓦利斯和富图纳
     */
    WF,
    /**
     * 🇪🇭 Western Sahara
     *
     * ---
     *
     * 西撒哈拉
     */
    EH,
    /**
     * 🇾🇪 Yemen
     *
     * ---
     *
     * 也门
     */
    YE,
    /**
     * 🇿🇲 Zambia
     *
     * ---
     *
     * 赞比亚
     */
    ZM,
    /**
     * 🇿🇼 Zimbabwe
     *
     * ---
     *
     * 津巴布韦
     */
    ZW,
}

impl RegionCode {
    pub fn is(region_code: String) -> bool {
        RegionCode::iter().any(|variant| variant.to_string() == region_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is() {
        assert!(RegionCode::is("CN".to_string()));
        assert!(RegionCode::is("US".to_string()));
        assert!(!RegionCode::is("AAA".to_string()));
    }
}
