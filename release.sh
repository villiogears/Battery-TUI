#!/bin/bash
# 使い方: 
# ./release.sh        (パッチバージョンを上げる: 0.1.0 -> 0.1.1)
# ./release.sh minor  (マイナーバージョンを上げる: 0.1.0 -> 0.2.0)
# ./release.sh major  (メジャーバージョンを上げる: 0.1.0 -> 1.0.0)

set -e

# gitの作業ディレクトリがクリーンか確認 (未追跡ファイルは無視)
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "エラー: 未コミットの変更があります。変更をコミットまたはスタッシュしてから実行してください。"
  exit 1
fi

BUMP_TYPE=${1:-patch}

# Cargo.toml から現在のバージョンを取得
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
echo "現在のバージョン: $CURRENT_VERSION"

# バージョンを Major, Minor, Patch に分割
IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

# バージョンの更新
case "$BUMP_TYPE" in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  patch|*)
    PATCH=$((PATCH + 1))
    ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"
echo "新しいバージョン: $NEW_VERSION"

# Cargo.toml のバージョンを書き換え (macOSとLinuxのsed両対応)
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak

# Cargo.lock も更新しておく
cargo check > /dev/null 2>&1

# Git コミットとタグ付け
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NEW_VERSION"
git tag "v$NEW_VERSION"

# リモートへプッシュ (コミットとタグの両方)
git push origin HEAD
git push origin "v$NEW_VERSION"

echo "🎉 v$NEW_VERSION のリリース処理が完了しました！ GitHub Actionsでビルドが開始されます。"
