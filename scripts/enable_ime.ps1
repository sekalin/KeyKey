# KeyKey Windows 11 Language Switcher Integration Script
$tip = '0404:{B892F1C2-88E4-4866-96D5-812C465EB86B}{6786A76E-6F3C-4318-A785-5AE4E8251268}'

Write-Host "Updating Windows User Language List..."
$list = Get-WinUserLanguageList
$zh = $list | Where-Object { $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' }

if (-not $zh) {
    Write-Host "Adding zh-Hant-TW language pack..."
    $zh = New-WinUserLanguageList "zh-Hant-TW"
    $list.Add($zh[0])
    $zh = $list | Where-Object { $_.LanguageTag -like 'zh-Hant*' -or $_.LanguageTag -like 'zh-TW*' }
}

if ($zh) {
    if ($zh.InputMethodTips -notcontains $tip) {
        $zh.InputMethodTips.Add($tip)
        Set-WinUserLanguageList $list -Force
        Write-Host "Added KeyKey TIP to active InputMethodTips list."
    } else {
        Write-Host "KeyKey TIP is already in InputMethodTips."
    }
}

# Ensure Registry entry in User Profile
Set-ItemProperty -Path "HKCU:\Control Panel\International\User Profile\zh-Hant-TW" -Name $tip -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue
Write-Host "Configured HKCU User Profile."
