#!/bin/bash

# Script to run database migrations for oz-monitor-orchestrator

set -e

echo "Running database migration: 002_add_trigger_scripts.sql"

# Get the database URL from .env
source .env

# Execute the migration
psql "$OZ_MONITOR_DATABASE_URL" < ../stellar-monitor-tenant-isolation/migrations/002_add_trigger_scripts.sql

echo "Migration completed successfully!"
echo ""
echo "The following table has been created:"
echo "- trigger_scripts: Stores trigger scripts in the database"
echo ""
echo "The following columns have been added:"
echo "- tenant_monitors.trigger_script_ids: Array of script IDs for monitors"
echo "- tenant_triggers.script_id: Reference to script in trigger_scripts table"