#!/bin/bash
# Pixcli Demo Script
# Record with: asciinema rec demo.cast -c "bash docs/demo.sh"

set -e

echo "🏦 Pixcli — Demo"
echo "=================="
echo ""
sleep 2

echo "# Check version"
pixcli --version
sleep 1

echo ""
echo "# Check balance"
pixcli balance --sandbox
sleep 2

echo ""
echo "# Create a Pix charge for R\$5.00"
pixcli charge create --amount 5.00 --description "Coffee ☕" --sandbox
sleep 3

echo ""
echo "# Generate a QR code (offline, no API needed)"
pixcli qr generate --key "+5511999999999" --amount 10.00 --name "DEMO" --city "SAO PAULO"
sleep 3

echo ""
echo "# List recent charges"
pixcli charge list --sandbox
sleep 2

echo ""
echo "# Same thing, but in JSON (for scripts and AI agents)"
pixcli charge list --sandbox --output json | head -20
sleep 2

echo ""
echo "# Check transaction history"
pixcli pix list --days 7 --sandbox
sleep 2

echo ""
echo "✅ That's Pixcli! Install: cargo install pixcli"
echo "📖 Docs: https://github.com/pixcli/pixcli"
