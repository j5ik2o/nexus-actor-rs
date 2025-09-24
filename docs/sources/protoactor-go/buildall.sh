go get ./...

# shellcheck disable=SC2044
for f in $(find . -name go.mod)
do (cd $(dirname $f) || exit; go mod tidy)
done


for f in $(find . -name build.sh)
do (cd $(dirname $f) || exit; echo $(dirname $f); ./build.sh)
done