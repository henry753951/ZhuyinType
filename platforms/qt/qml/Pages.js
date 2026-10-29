.pragma library

var sidebar = [
    {id: "intro", title: "介紹", section: "概覽", family: "overview"},
    {id: "analytics", title: "分析", section: "", family: "overview"},
    {id: "typing", title: "打字", section: "輸入", family: "input"},
    {id: "choosing", title: "選字", section: "", family: "input"},
    {id: "words", title: "字詞", section: "", family: "library"},
    {id: "appearance", title: "視窗與提示", section: "外觀", family: "appearance"},
    {id: "keys", title: "快捷鍵", section: "一般", family: "library"},
    {id: "advanced", title: "進階", section: "", family: "library"},
    {id: "about", title: "關於", section: "", family: "about"}
];
var subpages = [
    {id: "punctuation", title: "標點與符號", parent: "typing", family: "input"},
    {id: "typingKeys", title: "所有打字的按鍵", parent: "typing", family: "input"},
    {id: "fuzzy", title: "模糊音", parent: "choosing", family: "input"},
    {id: "context", title: "前文", parent: "choosing", family: "input"},
    {id: "learnedWords", title: "記住的字詞", parent: "words", family: "library"},
    {id: "phrases", title: "自訂詞", parent: "words", family: "library"},
    {id: "apps", title: "在候選字視窗顯示組字的 App", parent: "advanced", family: "library"},
    {id: "performance", title: "效能", parent: "analytics", family: "overview"},
    {id: "dataSources", title: "授權與資料來源", parent: "about", family: "about"}
];
var all = sidebar.concat(subpages);
function lookup(id) { return all.find(page => page.id === id) || sidebar[0]; }
