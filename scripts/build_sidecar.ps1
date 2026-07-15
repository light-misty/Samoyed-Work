# Samoyed Work Python Sidecar 鏋勫缓鑴氭湰
# 涓嬭浇 Python Embeddable Distribution + 瀹夎渚濊禆 + 澶嶅埗 sidecar 婧愮爜 + 缂栬瘧 .pyc 闅愯棌婧愮爜
# 鏈€缁堜骇鐗?sidecar_dist/ 閫氳繃 tauri.conf.json 鐨?bundle.resources 鎵撳寘鍒?NSIS 瀹夎鍖?#
# 涓氬姟浠ｇ爜淇濇姢锛氶櫎 main.py 澶栵紝鎵€鏈?.py 缂栬瘧涓?.pyc 骞跺垹闄ゆ簮鏂囦欢锛岄伩鍏嶆簮鐮佺洿鎺ユ毚闇?#
# 鐢ㄦ硶锛?#   powershell -ExecutionPolicy Bypass -File scripts/build_sidecar.ps1
#
# 缂撳瓨绛栫暐锛歅ython Embeddable zip 鍜?get-pip.py 涓嬭浇鍚庣紦瀛樺埌 scripts/.cache/锛?#   鍚庣画鏋勫缓鑷姩澶嶇敤缂撳瓨锛屽闇€寮哄埗閲嶆柊涓嬭浇璇峰垹闄?scripts/.cache/ 涓殑瀵瑰簲鏂囦欢

# 涓ユ牸閿欒妯″紡锛氫换浣曢敊璇兘缁堟鑴氭湰
$ErrorActionPreference = "Stop"

# ============================================
# 閰嶇疆椤?# ============================================

# Python 鐗堟湰锛堥」鐩姹?3.12+锛?.12.7 鏄?3.12 绯诲垪绋冲畾鐗堟湰锛?$PythonVersion = "3.12.7"
# Python Embeddable 涓嬭浇 URL锛堝井杞畼鏂?FTP锛?$PythonDownloadUrl = "https://www.python.org/ftp/python/$PythonVersion/python-$PythonVersion-embed-amd64.zip"
# get-pip.py 涓嬭浇 URL锛堝畼鏂?bootstrap锛?$GetPipUrl = "https://bootstrap.pypa.io/get-pip.py"
# PyPI 闀滃儚婧愶紙鍥藉唴鐢ㄦ埛鍔犻€燂紝娓呭崕 TUNA 闀滃儚锛?# 濡傞渶浣跨敤瀹樻柟婧愶紝鏀逛负 "https://pypi.org/simple"
$PyMirrorUrl = "https://pypi.tuna.tsinghua.edu.cn/simple"

# 璺緞閰嶇疆锛堝熀浜庤剼鏈墍鍦ㄤ綅缃帹瀵奸」鐩牴鐩綍锛?$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$DistDir = Join-Path $ProjectRoot "sidecar_dist"
$PythonDir = Join-Path $DistDir "python"
$SidecarSourceDir = Join-Path $ProjectRoot "sidecar"
$SidecarTargetDir = Join-Path $DistDir "sidecar"
$CacheDir = Join-Path $ScriptDir ".cache"
$PythonZipPath = Join-Path $CacheDir "python-$PythonVersion-embed-amd64.zip"
$GetPipPath = Join-Path $CacheDir "get-pip.py"
$RequirementsPath = Join-Path $SidecarSourceDir "requirements.txt"

# Python 鍙墽琛屾枃浠惰矾寰?$PythonExe = Join-Path $PythonDir "python.exe"
# python312._pth 鏂囦欢璺緞锛堢増鏈彿闇€涓?PythonVersion 涓绘鐗堟湰涓€鑷达級
$PythonMinorVersion = ($PythonVersion -split '\.')[0..1] -join '.'
$PthFile = Join-Path $PythonDir "python$($PythonMinorVersion.Replace('.', ''))._pth"

# ============================================
# 宸ュ叿鍑芥暟
# ============================================

function Write-Step {
    # 杈撳嚭姝ラ鏍囬锛堢豢鑹诧級
    param([string]$Message)
    Write-Host ""
    Write-Host "===========================================" -ForegroundColor Green
    Write-Host "  $Message" -ForegroundColor Green
    Write-Host "===========================================" -ForegroundColor Green
}

function Write-Info {
    # 杈撳嚭鏅€氫俊鎭紙鐧借壊锛?    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor White
}

function Write-Warn {
    # 杈撳嚭璀﹀憡锛堥粍鑹诧級
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Test-CommandSuccess {
    # 妫€鏌ヤ笂娆″懡浠ら€€鍑虹爜锛屽け璐ュ垯缁堟鑴氭湰
    param([string]$Context)
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] $Context 澶辫触锛岄€€鍑虹爜: $LASTEXITCODE" -ForegroundColor Red
        exit 1
    }
}

function Get-DirSizeMB {
    # 璁＄畻鐩綍浣撶Н锛圡B锛?    param([string]$Path)
    if (-not (Test-Path $Path)) { return 0 }
    $size = (Get-ChildItem $Path -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    return [math]::Round($size / 1MB, 2)
}

# ============================================
# 姝ラ 1锛氱幆澧冩鏌?# ============================================

Write-Step "姝ラ 1/9锛氱幆澧冩鏌?

# 闀滃儚婧愯繛閫氭€ф娴嬶細濡傛灉闀滃儚涓嶅彲杈撅紙濡?CI 鐜琚睆钄斤級锛屽洖閫€鍒板畼鏂?PyPI
try {
    $req = [System.Net.HttpWebRequest]::Create("$PyMirrorUrl/pip/")
    $req.Timeout = 5000
    $req.Method = "HEAD"
    $req.GetResponse().Close()
    Write-Info "闀滃儚婧愬彲杈? $PyMirrorUrl"
} catch {
    Write-Warn "闀滃儚婧愪笉鍙揪 ($($_.Exception.Message))锛屽洖閫€鍒板畼鏂?PyPI"
    $PyMirrorUrl = "https://pypi.org/simple"
}

# 妫€鏌?sidecar 婧愮爜鐩綍
if (-not (Test-Path $SidecarSourceDir)) {
    Write-Host "[ERROR] sidecar 婧愮爜鐩綍涓嶅瓨鍦? $SidecarSourceDir" -ForegroundColor Red
    exit 1
}
Write-Info "sidecar 婧愮爜鐩綍: $SidecarSourceDir"

# 妫€鏌?requirements.txt
if (-not (Test-Path $RequirementsPath)) {
    Write-Host "[ERROR] requirements.txt 涓嶅瓨鍦? $RequirementsPath" -ForegroundColor Red
    exit 1
}
Write-Info "渚濊禆娓呭崟: $RequirementsPath"

# 鍒涘缓缂撳瓨鐩綍
if (-not (Test-Path $CacheDir)) {
    New-Item -ItemType Directory -Path $CacheDir -Force | Out-Null
    Write-Info "鍒涘缓缂撳瓨鐩綍: $CacheDir"
}

# ============================================
# 姝ラ 2锛氭竻鐞嗘棫鐨勬瀯寤轰骇鐗?# ============================================

Write-Step "姝ラ 2/9锛氭竻鐞嗘棫鐨?sidecar_dist/"

if (Test-Path $DistDir) {
    Write-Info "鍒犻櫎鏃х殑 $DistDir"
    Remove-Item -Path $DistDir -Recurse -Force
}

New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
Write-Info "鍒涘缓 $DistDir"

# ============================================
# 姝ラ 3锛氫笅杞?Python Embeddable Distribution
# ============================================

Write-Step "姝ラ 3/9锛氫笅杞?Python $PythonVersion Embeddable"

# 缂撳瓨绛栫暐锛氬鏋?zip 宸插瓨鍦ㄥ垯澶嶇敤锛岄伩鍏嶉噸澶嶄笅杞?# 濡傞渶寮哄埗閲嶆柊涓嬭浇锛岃鍒犻櫎 scripts/.cache/ 涓殑缂撳瓨鏂囦欢
$NeedDownload = $true
if (Test-Path $PythonZipPath) {
    Write-Info "缂撳瓨宸插瓨鍦紝璺宠繃涓嬭浇: $PythonZipPath"
    Write-Info "濡傞渶寮哄埗閲嶆柊涓嬭浇锛岃鍏堝垹闄よ缂撳瓨鏂囦欢"
    $NeedDownload = $false
}

if ($NeedDownload) {
    Write-Info "涓嬭浇: $PythonDownloadUrl"
    Write-Info "鐩爣: $PythonZipPath"
    try {
        # 浣跨敤 .NET HttpClient 涓嬭浇锛屾敮鎸佸ぇ鏂囦欢鍜岃繘搴?        $ProgressPreference = 'Continue'
        Invoke-WebRequest -Uri $PythonDownloadUrl -OutFile $PythonZipPath -UseBasicParsing
        $ProgressPreference = 'SilentlyContinue'
    } catch {
        Write-Host "[ERROR] 涓嬭浇澶辫触: $_" -ForegroundColor Red
        exit 1
    }
    if (-not (Test-Path $PythonZipPath)) {
        Write-Host "[ERROR] 涓嬭浇鍚庢枃浠朵笉瀛樺湪: $PythonZipPath" -ForegroundColor Red
        exit 1
    }
    $zipSize = [math]::Round((Get-Item $PythonZipPath).Length / 1MB, 2)
    Write-Info "涓嬭浇瀹屾垚锛屼綋绉? $zipSize MB"
}

# ============================================
# 姝ラ 4锛氳В鍘?Python Embeddable
# ============================================

Write-Step "姝ラ 4/9锛氳В鍘?Python Embeddable"

Write-Info "瑙ｅ帇鍒? $PythonDir"
# 浣跨敤 Expand-Archive 瑙ｅ帇锛圥owerShell 鍐呯疆锛?Expand-Archive -Path $PythonZipPath -DestinationPath $PythonDir -Force

# 楠岃瘉 python.exe 瀛樺湪
if (-not (Test-Path $PythonExe)) {
    Write-Host "[ERROR] 瑙ｅ帇鍚庢湭鎵惧埌 python.exe: $PythonExe" -ForegroundColor Red
    exit 1
}
Write-Info "Python 瑙ｉ噴鍣? $PythonExe"

# 楠岃瘉鐗堟湰
$pythonVersionOutput = & $PythonExe --version 2>&1
Write-Info "Python 鐗堟湰: $pythonVersionOutput"

# ============================================
# 姝ラ 5锛氫慨鏀?python312._pth 鍚敤 site-packages
# ============================================

Write-Step "姝ラ 5/9锛氶厤缃?python._pth 鍚敤 site-packages"

# _pth 鏂囦欢鍚嶆牸寮忥細python312._pth锛堝幓鎺夌偣鐨勪富娆＄増鏈彿锛?if (-not (Test-Path $PthFile)) {
    Write-Host "[ERROR] _pth 鏂囦欢涓嶅瓨鍦? $PthFile" -ForegroundColor Red
    Write-Info "瀹為檯鏂囦欢鍒楄〃:"
    Get-ChildItem $PythonDir -Filter "python*._pth" | ForEach-Object { Write-Info "  $($_.Name)" }
    exit 1
}

Write-Info "_pth 鏂囦欢: $PthFile"
$pthContent = Get-Content $PthFile -Raw
Write-Info "鍘熷唴瀹?"
Write-Host $pthContent

# 鍙栨秷娉ㄩ噴 #import site锛堝惎鐢?site-packages 鑷姩鍔犺浇锛?# Python Embeddable 榛樿绂佺敤 site锛屽鑷?pip 瀹夎鐨勭涓夋柟搴撴棤娉?import
# 鍚敤鏂瑰紡锛氬彇娑堟敞閲?"import site" 琛岋紙鍘绘帀琛岄鐨?"# "锛?$newPthContent = $pthContent -replace '(?m)^#\s*import\s+site\s*$', 'import site'
$pthChanged = ($newPthContent -ne $pthContent)
if ($pthChanged) {
    Write-Info "宸插彇娑堟敞閲?'import site'"
} elseif ($pthContent -match '(?m)^import\s+site\s*$') {
    Write-Info "site 宸插惎鐢紙鏃犻渶淇敼锛?
} else {
    Write-Warn "鏈壘鍒?'#import site' 琛岋紝灏濊瘯杩藉姞 'import site'"
    $newPthContent = $pthContent.TrimEnd() + "`r`nimport site`r`n"
}

Set-Content -Path $PthFile -Value $newPthContent -NoNewline:$false
Write-Info "鏂板唴瀹?"
Write-Host $newPthContent

# ============================================
# 姝ラ 6锛氬畨瑁?pip 骞跺畨瑁呬緷璧?# ============================================

Write-Step "姝ラ 6/9锛氬畨瑁?pip 涓庣涓夋柟渚濊禆"

# 涓嬭浇 get-pip.py锛堝鏈紦瀛橈級
if (-not (Test-Path $GetPipPath)) {
    Write-Info "涓嬭浇 get-pip.py: $GetPipUrl"
    try {
        Invoke-WebRequest -Uri $GetPipUrl -OutFile $GetPipPath -UseBasicParsing
    } catch {
        Write-Host "[ERROR] 涓嬭浇 get-pip.py 澶辫触: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Info "浣跨敤缂撳瓨鐨?get-pip.py: $GetPipPath"
}

# 瀹氫箟瀹夎鍑芥暟锛屾敮鎸侀暅鍍忔簮涓嶅彲鐢ㄦ椂鑷姩鍥為€€鍒板畼鏂?PyPI
function Install-WithFallback {
    param([ScriptBlock]$ScriptBlock)
    # 灏濊瘯浣跨敤闀滃儚婧愬畨瑁?    $originalUrl = $PyMirrorUrl
    try {
        & $ScriptBlock
        if ($LASTEXITCODE -ne 0) { throw "閫€鍑虹爜: $LASTEXITCODE" }
    } catch {
        if ($PyMirrorUrl -ne "https://pypi.org/simple") {
            Write-Warn "闀滃儚婧愬畨瑁呭け璐?($($_.Exception.Message))锛屽洖閫€鍒板畼鏂?PyPI"
            $script:PyMirrorUrl = "https://pypi.org/simple"
            & $ScriptBlock
            Test-CommandSuccess "瀹夎锛堝畼鏂规簮锛?
            $script:PyMirrorUrl = $originalUrl
        } else {
            throw
        }
    }
}

# 浣跨敤宓屽叆寮?Python 鎵ц get-pip.py 瀹夎 pip
# 閫氳繃 --index-url 鎸囧畾闀滃儚婧愬姞閫?pip 鑷韩涓嬭浇
Write-Info "瀹夎 pip..."
Install-WithFallback -ScriptBlock {
    & $PythonExe $GetPipPath --no-warn-script-location --index-url $PyMirrorUrl
}
Test-CommandSuccess "瀹夎 pip"

# 楠岃瘉 pip 鍙敤
Write-Info "楠岃瘉 pip..."
& $PythonExe -m pip --version
Test-CommandSuccess "楠岃瘉 pip"

# 瀹夎 requirements.txt 涓殑渚濊禆
# 閫氳繃 -i 鎸囧畾闀滃儚婧愬姞閫熶緷璧栦笅杞?Write-Info "瀹夎渚濊禆: $RequirementsPath"
Write-Info "浣跨敤闀滃儚婧? $PyMirrorUrl"
Install-WithFallback -ScriptBlock {
    & $PythonExe -m pip install -r $RequirementsPath --no-warn-script-location -i $PyMirrorUrl
}
Test-CommandSuccess "瀹夎 Python 渚濊禆"

# ============================================
# 姝ラ 7锛氬鍒?sidecar 婧愮爜
# ============================================

Write-Step "姝ラ 7/9锛氬鍒?sidecar 婧愮爜鍒?sidecar_dist/sidecar/"

Write-Info "婧? $SidecarSourceDir"
Write-Info "鐩爣: $SidecarTargetDir"

# 浣跨敤 robocopy 澶嶅埗锛堣嚜鍔ㄦ帓闄ょ紦瀛樼洰褰曞拰娴嬭瘯鐩綍锛屾瘮 Copy-Item 鏇撮珮鏁堬級
# /E 澶嶅埗瀛愮洰褰曪紙鍖呮嫭绌虹洰褰曪級
# /XD 鎺掗櫎鐩綍锛坃_pycache__/.pytest_cache/.cache 缂撳瓨锛泃ests 娴嬭瘯浠ｇ爜涓嶅簲鎵撳寘锛?# /XF 鎺掗櫎鏂囦欢锛?pyc/.pyo 缂撳瓨锛況equirements.txt 鏄瀯寤烘椂渚濊禆娓呭崟锛屽畨瑁呯洰褰曟棤闇€淇濈暀锛?# /NFL /NDL 涓嶆樉绀烘枃浠?鐩綍鍚嶏紙鍑忓皯杈撳嚭锛?# /NJH /NJS 涓嶆樉绀轰綔涓氬ご/鎽樿
# /NP 涓嶆樉绀鸿繘搴︾櫨鍒嗘瘮
$robocopyArgs = @(
    $SidecarSourceDir,
    $SidecarTargetDir,
    "/E",
    "/XD", "__pycache__", ".pytest_cache", ".cache", "tests",
    "/XF", "*.pyc", "*.pyo", "requirements.txt",
    "/NFL", "/NDL", "/NJH", "/NJS", "/NP"
)
& robocopy @robocopyArgs

# robocopy 閫€鍑虹爜 0-7 閮芥槸鎴愬姛鐨勶紙<8 琛ㄧず鎴愬姛锛?=8 琛ㄧず澶辫触锛?# 0: 鏃犲彉鍖?1: 澶嶅埗鎴愬姛 2: 棰濆鏂囦欢 3: 1+2 4: 涓嶅尮閰?5-7: 缁勫悎
if ($LASTEXITCODE -ge 8) {
    Write-Host "[ERROR] robocopy 澶辫触锛岄€€鍑虹爜: $LASTEXITCODE" -ForegroundColor Red
    exit 1
}
# 閲嶇疆 $LASTEXITCODE锛坮obocopy 闈為浂閫€鍑虹爜鍙兘褰卞搷鍚庣画鍒ゆ柇锛?$global:LASTEXITCODE = 0

Write-Info "sidecar 婧愮爜澶嶅埗瀹屾垚"

# 楠岃瘉 main.py 瀛樺湪
$MainPy = Join-Path $SidecarTargetDir "main.py"
if (-not (Test-Path $MainPy)) {
    Write-Host "[ERROR] 澶嶅埗鍚庢湭鎵惧埌 main.py: $MainPy" -ForegroundColor Red
    exit 1
}

# ============================================
# 姝ラ 8锛氱紪璇戜笟鍔?.py 涓?.pyc 骞跺垹闄?.py 婧愭枃浠?# ============================================

Write-Step "姝ラ 8/9锛氱紪璇戜笟鍔′唬鐮佷负 .pyc锛堥殣钘忔簮鐮侊級"

# 浣跨敤 compileall 缂栬瘧鎵€鏈?.py 涓?.pyc
# -b 鍙傛暟锛氬皢 .pyc 鏂囦欢鏀惧湪涓?.py 鍚岀骇鐨勭洰褰曪紙鑰屼笉鏄?__pycache__ 瀛愮洰褰曪級
# 杩欐牱鍒犻櫎 .py 鍚庯紝Python 浼氱洿鎺ュ姞杞藉悓绾х殑 .pyc锛圥ython 2 閬楃暀琛屼负锛?.x 浠嶆敮鎸侊級
# -q 鍙傛暟锛氶潤榛樻ā寮忥紝鍙緭鍑洪敊璇?Write-Info "缂栬瘧 .py 涓?.pyc..."
& $PythonExe -m compileall -b -q $SidecarTargetDir
Test-CommandSuccess "缂栬瘧 .pyc"

# 缁熻缂栬瘧鐢熸垚鐨?.pyc 鏂囦欢鏁伴噺
$pycCount = (Get-ChildItem -Path $SidecarTargetDir -Recurse -Filter "*.pyc" -File).Count
Write-Info "宸茬敓鎴?$pycCount 涓?.pyc 鏂囦欢"

# 鍒犻櫎 .py 婧愭枃浠讹紝淇濈暀 main.py锛堝叆鍙ｏ級鍜?__init__.py锛圥ython 3 鍖呭垵濮嬪寲蹇呴渶锛?# main.py锛氬叆鍙ｆ枃浠讹紝python.exe main.py 闇€瑕佸畠锛屽彧鏄畝鍗曡皟搴﹂€昏緫
# __init__.py锛歅ython 3 璇嗗埆鏅€氬寘鐨勫繀闇€鏂囦欢锛屽彧鏈?__init__.pyc 鏃犳硶琚瘑鍒负鍖?#   - handlers/__init__.py锛氬彧鏈変竴琛屾敞閲?# 鏍稿績涓氬姟閫昏緫閮藉湪 handlers/*.pyc 涓紙瀛楄妭鐮侊紝鏃犳硶鐢ㄨ浜嬫湰鏌ョ湅锛?Write-Info "鍒犻櫎涓氬姟 .py 婧愭枃浠讹紙淇濈暀 main.py 鍜?__init__.py锛?.."
$deletedCount = 0
Get-ChildItem -Path $SidecarTargetDir -Recurse -Filter "*.py" -File | Where-Object { $_.Name -notin @("main.py", "__init__.py") } | ForEach-Object {
    Remove-Item -Path $_.FullName -Force
    $deletedCount++
}
Write-Info "宸插垹闄?$deletedCount 涓?.py 鏂囦欢锛堜繚鐣?main.py 鍜?__init__.py锛?

# 娓呯悊 compileall 鍙兘鐢熸垚鐨?__pycache__ 鐩綍锛堜娇鐢?-b 鍚?.pyc 宸插湪鍚岀骇锛宊_pycache__ 涓虹┖鎴栧啑浣欙級
$pycacheDir = Join-Path $SidecarTargetDir "__pycache__"
if (Test-Path $pycacheDir) {
    Remove-Item -Path $pycacheDir -Recurse -Force
}
# 閫掑綊娓呯悊瀛愮洰褰曚笅鐨?__pycache__
Get-ChildItem -Path $SidecarTargetDir -Recurse -Directory -Filter "__pycache__" | ForEach-Object {
    Remove-Item -Path $_.FullName -Recurse -Force
}
Write-Info "宸叉竻鐞?__pycache__ 鐩綍"

# 楠岃瘉 .pyc 鏂囦欢缁撴瀯锛堝垪鍑哄叧閿枃浠讹級
Write-Info "涓氬姟浠ｇ爜鏂囦欢缁撴瀯:"
Get-ChildItem -Path $SidecarTargetDir -Recurse -File | Where-Object { $_.Extension -in ".py", ".pyc" } | ForEach-Object {
    $relPath = $_.FullName.Replace($SidecarTargetDir, "").TrimStart("\")
    Write-Info "  $relPath"
}

# ============================================
# 姝ラ 9锛氶獙璇?sidecar 鍙惎鍔?# ============================================

Write-Step "姝ラ 9/9锛氶獙璇?sidecar 閫氫俊"

# 鍚姩 sidecar 杩涚▼锛屽彂閫?ping 璇锋眰楠岃瘉閫氫俊
Write-Info "鍚姩 sidecar 杩涚▼杩涜楠岃瘉..."
$process = New-Object System.Diagnostics.Process
$process.StartInfo.FileName = $PythonExe
$process.StartInfo.Arguments = "`"$MainPy`""
$process.StartInfo.UseShellExecute = $false
$process.StartInfo.RedirectStandardInput = $true
$process.StartInfo.RedirectStandardOutput = $true
$process.StartInfo.RedirectStandardError = $true
$process.StartInfo.CreateNoWindow = $true

try {
    $process.Start() | Out-Null
} catch {
    Write-Host "[ERROR] 鍚姩 sidecar 杩涚▼澶辫触: $_" -ForegroundColor Red
    exit 1
}

# 鍙戦€?ping 璇锋眰
$pingRequest = '{"id":"buildtest","action":"ping","type":"health"}'
Write-Info "鍙戦€?ping 璇锋眰: $pingRequest"
$process.StandardInput.WriteLine($pingRequest)
$process.StandardInput.Flush()

# 璇诲彇鍝嶅簲锛? 绉掕秴鏃讹級
$process.WaitForExit(5000) | Out-Null
$response = $null
if (-not $process.HasExited) {
    # 灏濊瘯璇诲彇鍝嶅簲琛岋紙甯﹁秴鏃讹級
    $readTask = $process.StandardOutput.ReadLineAsync()
    $readTask.Wait(5000) | Out-Null
    if ($readTask.IsCompleted) {
        $response = $readTask.Result
    }
}

# 缁堟 sidecar 杩涚▼
# 蹇呴』绛夊緟杩涚▼瀹屽叏閫€鍑哄悗鍐嶆竻鐞嗘棩蹇楁枃浠讹紝鍚﹀垯鏂囦欢鍙ユ焺鏈噴鏀句細瀵艰嚧娓呯悊澶辫触
if (-not $process.HasExited) {
    try {
        $process.Kill()
        # WaitForExit 绛夊緟杩涚▼瀹屽叏閫€鍑哄苟閲婃斁鏂囦欢鍙ユ焺锛堣秴鏃?3 绉掍繚鎶わ級
        $process.WaitForExit(3000) | Out-Null
    } catch { }
}

if ($null -eq $response) {
    Write-Host "[ERROR] 鏈敹鍒?sidecar 鍝嶅簲锛堣秴鏃讹級" -ForegroundColor Red
    Write-Info "stderr 杈撳嚭:"
    $stderr = $process.StandardError.ReadToEnd()
    Write-Host $stderr
    exit 1
}

Write-Info "鍝嶅簲: $response"

# 楠岃瘉鍝嶅簲鍖呭惈 success: true
if ($response -match '"success"\s*:\s*true') {
    Write-Info "sidecar 閫氫俊楠岃瘉閫氳繃"
} else {
    Write-Host "[ERROR] sidecar 鍝嶅簲寮傚父: $response" -ForegroundColor Red
    exit 1
}

# 娓呯悊楠岃瘉閫氫俊鏃?sidecar 杩涚▼鍦?sidecar_dist/log/ 涓嬬敓鎴愮殑鏃ュ織鏂囦欢
# 杩欎簺鏃ュ織鏄瀯寤洪獙璇佷骇鐗╋紝涓嶅簲鎵撳寘鍒板畨瑁呭寘涓?# 锛堢敓浜х幆澧冭繍琛屾椂锛宻idecar 鏃ュ織鐢?Rust 绔€氳繃 SAMOYED_WORK_LOG_DIR 鐜鍙橀噺
#   鎸囧悜 %LOCALAPPDATA%\samoyed-work\logs\锛屼笉浼氬啓鍒板畨瑁呯洰褰曪級
$buildLogDir = Join-Path $DistDir "log"
if (Test-Path $buildLogDir) {
    Write-Info "娓呯悊鏋勫缓楠岃瘉鏃ュ織鐩綍: $buildLogDir"
    # 閲嶈瘯鏈哄埗锛歴idecar 杩涚▼鐨?FileHandler 鍙兘寤惰繜閲婃斁鏂囦欢鍙ユ焺
    # 绛夊緟 500ms 鍚庨噸璇曪紝鏈€澶?3 娆?    $removed = $false
    for ($i = 1; $i -le 3; $i++) {
        try {
            Remove-Item -Path $buildLogDir -Recurse -Force -ErrorAction Stop
            $removed = $true
            break
        } catch {
            if ($i -lt 3) {
                Write-Info "娓呯悊鏃ュ織鐩綍澶辫触锛堢 $i 娆★級锛岀瓑寰?500ms 鍚庨噸璇? $_"
                Start-Sleep -Milliseconds 500
            }
        }
    }
    if (-not $removed) {
        Write-Warn "娓呯悊鏃ュ織鐩綍澶辫触锛堝凡閲嶈瘯 3 娆★級锛岃烦杩囨竻鐞? $buildLogDir"
        Write-Warn "涓嬫鏋勫缓姝ラ 2 浼氭竻鐞嗘暣涓?sidecar_dist 鐩綍锛屼笉褰卞搷鏋勫缓姝ｇ‘鎬?
    }
}

# 娓呯悊楠岃瘉閫氫俊鏃?Python 鍔犺浇 __init__.py 鑷姩鐢熸垚鐨?__pycache__ 鐩綍
# 锛堟楠?8 宸叉竻鐞嗚繃 compileall 浜х敓鐨?__pycache__锛屼絾姝ラ 9 鍚姩 sidecar 楠岃瘉鏃?#   Python 浼氶噸鏂扮敓鎴?handlers/__pycache__/__init__.cpython-312.pyc锛?$pycacheDirs = Get-ChildItem -Path $SidecarTargetDir -Recurse -Directory -Filter "__pycache__" -ErrorAction SilentlyContinue
if ($pycacheDirs) {
    Write-Info "娓呯悊楠岃瘉浜х敓鐨?__pycache__ 鐩綍..."
    $pycacheDirs | ForEach-Object {
        Remove-Item -Path $_.FullName -Recurse -Force
    }
}

# ============================================
# 鏋勫缓缁撴灉缁熻
# ============================================

Write-Step "鏋勫缓瀹屾垚"

$distSize = Get-DirSizeMB -Path $DistDir
$pythonSize = Get-DirSizeMB -Path $PythonDir
$sidecarSize = Get-DirSizeMB -Path $SidecarTargetDir

Write-Info "sidecar_dist 鎬讳綋绉? $distSize MB"
Write-Info "  - python/ (瑙ｉ噴鍣?渚濊禆): $pythonSize MB"
Write-Info "  - sidecar/ (涓氬姟浠ｇ爜): $sidecarSize MB"
Write-Info "浜х墿璺緞: $DistDir"
Write-Host ""
Write-Host "涓嬩竴姝ワ細杩愯 'npm run tauri:build' 鏋勫缓 NSIS 瀹夎鍖? -ForegroundColor Cyan

