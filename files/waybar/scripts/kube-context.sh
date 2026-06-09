#!/usr/bin/env bash

if ! command -v kubectl >/dev/null 2>&1; then
    echo "󱃾 NA"
    exit 0
fi

context=$(kubectl config current-context 2>/dev/null)

if [ -z "$context" ]; then
    echo "󱃾 none"
    exit 0
fi

case "$context" in
    *prod*|*production*)
        icon="󱃾 PROD"
        ;;
    *stage*|*staging*)
        icon="󱃾 STG"
        ;;
    *dev*|*development*)
        icon="󱃾 DEV"
        ;;
    *minikube*)
        icon="󱃾 mini"
        ;;
    *kind*)
        icon="󱃾 kind"
        ;;
    *)
        icon="󱃾"
        ;;
esac

short=$(echo "$context" | sed 's/^arn:aws:eks:[^:]*:[^:]*:cluster\///' | cut -c1-18)

echo "$icon $short"
