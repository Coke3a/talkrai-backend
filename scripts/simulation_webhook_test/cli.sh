#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Simulation Webhook Test CLI (Bash version)
#
# Dependencies: psql
# Usage:        ./cli.sh
# =============================================================================

# ---------------------------------------------------------------------------
# ANSI colors
# ---------------------------------------------------------------------------
BOLD='\033[1m'
DIM='\033[2m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
RESET='\033[0m'

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

load_env() {
    # Load .env file (local takes precedence)
    local env_file="${SCRIPT_DIR}/.env"
    if [[ -f "$env_file" ]]; then
        set -a
        # shellcheck disable=SC1090
        source "$env_file"
        set +a
    fi

    if [[ -z "${DATABASE_URL:-}" ]]; then
        echo -e "${RED}DATABASE_URL not found.${RESET}"
        echo "Create scripts/simulation_webhook_test/.env from .env.example"
        exit 1
    fi
}

check_deps() {
    local missing=()
    command -v psql  >/dev/null 2>&1 || missing+=(psql)

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo -e "${RED}Missing dependencies:${RESET} ${missing[*]}"
        echo "Install with: sudo apt install ${missing[*]}"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# DB helpers
# ---------------------------------------------------------------------------
db_query() {
    # Machine-readable: tab-separated, no headers, no alignment
    psql "$DATABASE_URL" -t -A -F $'\t' -c "$1" 2>/dev/null
}

db_query_table() {
    # Human-readable: bordered table with headers
    psql "$DATABASE_URL" --pset=border=1 --pset=format=aligned -c "$1" 2>/dev/null
}

# ---------------------------------------------------------------------------
# Panel display (replaces Rich Panel)
# ---------------------------------------------------------------------------
print_panel() {
    local title="$1"
    local color="$2"
    shift 2
    local lines=("$@")

    # Find max line width (strip ANSI for measurement)
    local max_len=${#title}
    for line in "${lines[@]}"; do
        local stripped
        stripped=$(echo -e "$line" | sed 's/\x1b\[[0-9;]*m//g')
        local len=${#stripped}
        (( len > max_len )) && max_len=$len
    done
    local width=$(( max_len + 4 ))

    # Draw box
    local hline=""
    for (( i=0; i<width; i++ )); do hline+="─"; done

    echo -e "${color}┌─ ${BOLD}${title}${RESET}${color} $(printf '─%.0s' $(seq 1 $(( width - ${#title} - 3 ))))┐${RESET}"
    for line in "${lines[@]}"; do
        local stripped
        stripped=$(echo -e "$line" | sed 's/\x1b\[[0-9;]*m//g')
        local pad=$(( width - ${#stripped} - 2 ))
        local spaces=""
        for (( i=0; i<pad; i++ )); do spaces+=" "; done
        echo -e "${color}│${RESET} ${line}${spaces} ${color}│${RESET}"
    done
    echo -e "${color}└${hline}┘${RESET}"
}

# ---------------------------------------------------------------------------
# Info panels
# ---------------------------------------------------------------------------
print_user_info() {
    local user_id="$1" line_id="$2" display_name="$3" terms_at="$4"
    local terms_display
    if [[ -n "$terms_at" && "$terms_at" != "null" ]]; then
        terms_display="$terms_at"
    else
        terms_display="${YELLOW}Not accepted${RESET}"
    fi
    print_panel "User" "$BLUE" \
        "${BOLD}ID:${RESET}            $user_id" \
        "${BOLD}LINE ID:${RESET}       $line_id" \
        "${BOLD}Display Name:${RESET}  ${display_name:-unnamed}" \
        "${BOLD}Terms:${RESET}         $terms_display"
}

print_session_info() {
    if [[ -z "${SESSION_ID:-}" ]]; then
        print_panel "Session" "$DIM" "${DIM}No active session${RESET}"
        return
    fi
    print_panel "Active Session" "$GREEN" \
        "${BOLD}Session ID:${RESET}      $SESSION_ID" \
        "${BOLD}Character:${RESET}       ${SESSION_CHAR_NAME}" \
        "${BOLD}Scene:${RESET}           ${SESSION_SCENE_NAME}" \
        "${BOLD}Status:${RESET}          ${SESSION_STATUS}" \
        "${BOLD}Mood:${RESET}            ${SESSION_MOOD}" \
        "${BOLD}Relationship:${RESET}    ${SESSION_REL}" \
        "${BOLD}Messages:${RESET}        ${SESSION_MSG_COUNT}"
}

print_credit_info() {
    if [[ -z "${CREDIT_BALANCE:-}" ]]; then
        print_panel "Credits" "$DIM" "${DIM}No credit balance${RESET}"
        return
    fi
    print_panel "Credits" "$YELLOW" \
        "${BOLD}Balance:${RESET}         $CREDIT_BALANCE" \
        "${BOLD}Purchased:${RESET}       $CREDIT_PURCHASED" \
        "${BOLD}Consumed:${RESET}        $CREDIT_CONSUMED"
}

# ---------------------------------------------------------------------------
# Query helpers (set global vars)
# ---------------------------------------------------------------------------

# Session globals
SESSION_ID="" SESSION_CHAR_ID="" SESSION_SCENE_ID=""
SESSION_CHAR_NAME="" SESSION_SCENE_NAME="" SESSION_STATUS=""
SESSION_MOOD="" SESSION_REL="" SESSION_MSG_COUNT=""

get_active_session() {
    local user_id="$1"
    SESSION_ID="" SESSION_CHAR_ID="" SESSION_SCENE_ID=""
    SESSION_CHAR_NAME="" SESSION_SCENE_NAME="" SESSION_STATUS=""
    SESSION_MOOD="" SESSION_REL="" SESSION_MSG_COUNT=""

    local row
    row=$(db_query "
        SELECT rs.id, rs.character_id, rs.scene_id,
               c.name, sc.name, rs.status,
               rs.mood, rs.relationship_level, rs.message_count
        FROM roleplay_sessions rs
        JOIN characters c ON c.id = rs.character_id
        JOIN scenes sc ON sc.id = rs.scene_id
        WHERE rs.user_id = '$user_id' AND rs.status = 'active'
        LIMIT 1
    ")
    [[ -z "$row" ]] && return 1

    IFS=$'\t' read -r SESSION_ID SESSION_CHAR_ID SESSION_SCENE_ID \
        SESSION_CHAR_NAME SESSION_SCENE_NAME SESSION_STATUS \
        SESSION_MOOD SESSION_REL SESSION_MSG_COUNT <<< "$row"
    return 0
}

# Credit globals
CREDIT_BALANCE="" CREDIT_PURCHASED="" CREDIT_CONSUMED=""

get_credit_balance() {
    local user_id="$1"
    CREDIT_BALANCE="" CREDIT_PURCHASED="" CREDIT_CONSUMED=""

    local row
    row=$(db_query "
        SELECT balance, total_purchased, total_consumed
        FROM credit_balances WHERE user_id = '$user_id'
    ")
    [[ -z "$row" ]] && return 1

    IFS=$'\t' read -r CREDIT_BALANCE CREDIT_PURCHASED CREDIT_CONSUMED <<< "$row"
    return 0
}

# ---------------------------------------------------------------------------
# Composite status
# ---------------------------------------------------------------------------
print_user_status() {
    local user_id="$1" line_id="$2" display_name="$3" terms_at="$4"
    print_user_info "$user_id" "$line_id" "$display_name" "$terms_at"
    get_active_session "$user_id" || true
    print_session_info
    get_credit_balance "$user_id" || true
    print_credit_info
}

# ---------------------------------------------------------------------------
# Selection globals
# ---------------------------------------------------------------------------
SELECTED_USER_ID="" SELECTED_LINE_ID="" SELECTED_DISPLAY_NAME="" SELECTED_TERMS_AT=""
SELECTED_CHAR_ID="" SELECTED_SCENE_ID="" SELECTED_SCENE_NAME=""
SELECTED_MOOD="" SELECTED_REL="" SELECTED_CHAR_NAME="" SELECTED_LOCATION=""

select_user() {
    SELECTED_USER_ID="" SELECTED_LINE_ID="" SELECTED_DISPLAY_NAME="" SELECTED_TERMS_AT=""

    local rows
    rows=$(db_query "
        SELECT u.id, u.line_user_id, COALESCE(u.display_name, 'unnamed'),
               COALESCE(u.terms_accepted_at::text, ''), COALESCE(cb.balance::text, '-')
        FROM users u
        LEFT JOIN credit_balances cb ON cb.user_id = u.id
        ORDER BY u.created_at DESC
        LIMIT 30
    ")

    if [[ -z "$rows" ]]; then
        echo -e "${YELLOW}No users found.${RESET}"
        return 1
    fi

    # Build parallel arrays for bash select
    local labels=() uids=() line_ids=() dnames=() terms_ats=()
    while IFS=$'\t' read -r uid line_id dname terms_at balance; do
        local short_id="${uid:0:8}"
        labels+=("${dname} (${short_id}...) [LINE: ${line_id}] credits:${balance}")
        uids+=("$uid")
        line_ids+=("$line_id")
        dnames+=("$dname")
        terms_ats+=("$terms_at")
    done <<< "$rows"

    echo -e "${BOLD}Select user:${RESET}"
    local PS3="Enter number (or $(( ${#labels[@]} + 1 )) to cancel): "
    select choice in "${labels[@]}" "Cancel"; do
        if [[ "$choice" == "Cancel" ]]; then
            return 1
        fi
        if [[ -n "$choice" ]]; then
            local idx=$(( REPLY - 1 ))
            SELECTED_USER_ID="${uids[$idx]}"
            SELECTED_LINE_ID="${line_ids[$idx]}"
            SELECTED_DISPLAY_NAME="${dnames[$idx]}"
            SELECTED_TERMS_AT="${terms_ats[$idx]}"
            return 0
        fi
    done
    return 1
}

select_scene() {
    SELECTED_CHAR_ID="" SELECTED_SCENE_ID="" SELECTED_SCENE_NAME=""
    SELECTED_MOOD="" SELECTED_REL="" SELECTED_CHAR_NAME="" SELECTED_LOCATION=""

    local rows
    rows=$(db_query "
        SELECT c.id, c.name, s.id, s.name, s.location,
               s.start_mood, s.start_relationship_level
        FROM characters c
        JOIN scenes s ON s.character_id = c.id
        WHERE c.is_active = true AND s.is_active = true
        ORDER BY c.name, s.name
    ")

    if [[ -z "$rows" ]]; then
        echo -e "${YELLOW}No active scenes found.${RESET}"
        return 1
    fi

    # Build parallel arrays for bash select
    local labels=() char_ids=() scene_ids=() scene_names=()
    local moods=() rels=() char_names=() locations=()
    while IFS=$'\t' read -r char_id char_name scene_id scene_name location mood rel; do
        labels+=("${char_name} - ${scene_name} (${location})")
        char_ids+=("$char_id")
        scene_ids+=("$scene_id")
        scene_names+=("$scene_name")
        moods+=("$mood")
        rels+=("$rel")
        char_names+=("$char_name")
        locations+=("$location")
    done <<< "$rows"

    echo -e "${BOLD}Select scene:${RESET}"
    local PS3="Enter number (or $(( ${#labels[@]} + 1 )) to cancel): "
    select choice in "${labels[@]}" "Cancel"; do
        if [[ "$choice" == "Cancel" ]]; then
            return 1
        fi
        if [[ -n "$choice" ]]; then
            local idx=$(( REPLY - 1 ))
            SELECTED_CHAR_ID="${char_ids[$idx]}"
            SELECTED_SCENE_ID="${scene_ids[$idx]}"
            SELECTED_SCENE_NAME="${scene_names[$idx]}"
            SELECTED_MOOD="${moods[$idx]}"
            SELECTED_REL="${rels[$idx]}"
            SELECTED_CHAR_NAME="${char_names[$idx]}"
            SELECTED_LOCATION="${locations[$idx]}"
            return 0
        fi
    done
    return 1
}

# ---------------------------------------------------------------------------
# Actions
# ---------------------------------------------------------------------------

action_list_users() {
    echo -e "\n${BOLD}Users (latest 20)${RESET}"
    db_query_table "
        SELECT
            u.id            AS \"User ID\",
            u.line_user_id  AS \"LINE ID\",
            COALESCE(u.display_name, '-') AS \"Display Name\",
            CASE WHEN u.terms_accepted_at IS NOT NULL THEN 'Yes' ELSE 'No' END AS \"Terms\",
            COALESCE(cb.balance::text, '-') AS \"Credits\",
            COALESCE(c.name || ' / ' || sc.name, '-') AS \"Active Session\"
        FROM users u
        LEFT JOIN credit_balances cb ON cb.user_id = u.id
        LEFT JOIN roleplay_sessions rs ON rs.user_id = u.id AND rs.status = 'active'
        LEFT JOIN characters c ON c.id = rs.character_id
        LEFT JOIN scenes sc ON sc.id = rs.scene_id
        ORDER BY u.created_at DESC
        LIMIT 20
    "
}

action_list_scenes() {
    echo -e "\n${BOLD}Active Characters & Scenes${RESET}"
    db_query_table "
        SELECT
            c.name                      AS \"Character\",
            c.gender                    AS \"Gender\",
            s.id                        AS \"Scene ID\",
            s.name                      AS \"Scene Name\",
            s.location                  AS \"Location\",
            s.start_mood                AS \"Mood\",
            s.start_relationship_level  AS \"Relationship\"
        FROM characters c
        JOIN scenes s ON s.character_id = c.id
        WHERE c.is_active = true AND s.is_active = true
        ORDER BY c.name, s.name
    "
    local count
    count=$(db_query "
        SELECT COUNT(*)
        FROM characters c
        JOIN scenes s ON s.character_id = c.id
        WHERE c.is_active = true AND s.is_active = true
    ")
    echo -e "${DIM}${count} scenes total${RESET}"
}

action_show_status() {
    select_user || return
    local user_id="$SELECTED_USER_ID"

    echo ""
    print_user_status "$user_id" "$SELECTED_LINE_ID" "$SELECTED_DISPLAY_NAME" "$SELECTED_TERMS_AT"

    # Recent messages
    echo ""
    local msg_rows
    msg_rows=$(db_query "
        SELECT m.role, m.message_type,
               CASE WHEN LENGTH(m.content) > 60
                    THEN SUBSTRING(m.content, 1, 57) || '...'
                    ELSE m.content END,
               TO_CHAR(m.created_at, 'HH24:MI:SS')
        FROM messages m
        JOIN roleplay_sessions rs ON rs.id = m.session_id
        WHERE rs.user_id = '$user_id'
        ORDER BY m.created_at DESC
        LIMIT 5
    ")

    if [[ -n "$msg_rows" ]]; then
        echo -e "${BOLD}Recent Messages (latest 5)${RESET}"
        db_query_table "
            SELECT
                TO_CHAR(m.created_at, 'HH24:MI:SS') AS \"Time\",
                m.role                               AS \"Role\",
                m.message_type                       AS \"Type\",
                CASE WHEN LENGTH(m.content) > 60
                     THEN SUBSTRING(m.content, 1, 57) || '...'
                     ELSE m.content END              AS \"Content\"
            FROM messages m
            JOIN roleplay_sessions rs ON rs.id = m.session_id
            WHERE rs.user_id = '$user_id'
            ORDER BY m.created_at DESC
            LIMIT 5
        "
    else
        echo -e "${DIM}No messages yet.${RESET}"
    fi

    # Recent jobs
    echo ""
    local job_rows
    job_rows=$(db_query "
        SELECT id FROM jobs WHERE user_id = '$user_id' LIMIT 1
    ")

    if [[ -n "$job_rows" ]]; then
        echo -e "${BOLD}Recent Jobs (latest 3)${RESET}"
        db_query_table "
            SELECT
                SUBSTRING(id::text, 1, 8) || '...' AS \"Job ID\",
                mode                               AS \"Mode\",
                status                             AS \"Status\",
                attempts                           AS \"Attempts\",
                TO_CHAR(created_at, 'HH24:MI:SS')  AS \"Created\",
                COALESCE(
                    CASE WHEN LENGTH(failed_reason) > 40
                         THEN SUBSTRING(failed_reason, 1, 37) || '...'
                         ELSE failed_reason END,
                    ''
                )                                  AS \"Error\"
            FROM jobs
            WHERE user_id = '$user_id'
            ORDER BY created_at DESC
            LIMIT 3
        "
    else
        echo -e "${DIM}No jobs yet.${RESET}"
    fi
}

action_accept_terms() {
    select_user || return

    if [[ -n "$SELECTED_TERMS_AT" ]]; then
        echo -e "${YELLOW}User already accepted terms at ${SELECTED_TERMS_AT}${RESET}"
        print_user_status "$SELECTED_USER_ID" "$SELECTED_LINE_ID" "$SELECTED_DISPLAY_NAME" "$SELECTED_TERMS_AT"
        return
    fi

    db_query "
        UPDATE users
        SET terms_accepted_at = NOW(), updated_at = NOW()
        WHERE id = '$SELECTED_USER_ID' AND terms_accepted_at IS NULL
    "

    echo -e "${GREEN}Terms accepted successfully.${RESET}"
    # Refresh terms_at for display
    SELECTED_TERMS_AT=$(db_query "SELECT terms_accepted_at::text FROM users WHERE id = '$SELECTED_USER_ID'")
    print_user_status "$SELECTED_USER_ID" "$SELECTED_LINE_ID" "$SELECTED_DISPLAY_NAME" "$SELECTED_TERMS_AT"
}

action_create_session() {
    select_user || return
    local user_id="$SELECTED_USER_ID"
    local line_id="$SELECTED_LINE_ID"
    local dname="$SELECTED_DISPLAY_NAME"
    local terms_at="$SELECTED_TERMS_AT"

    if [[ -z "$terms_at" ]]; then
        echo -e "${RED}User has not accepted terms yet.${RESET}"
        echo -e "${DIM}Use 'Accept Terms' first.${RESET}"
        return
    fi

    select_scene || return
    local char_id="$SELECTED_CHAR_ID"
    local scene_id="$SELECTED_SCENE_ID"
    local mood="$SELECTED_MOOD"
    local rel="$SELECTED_REL"

    # Transaction: end previous + create new
    local result
    result=$(psql "$DATABASE_URL" -t -A -F $'\t' <<SQL
BEGIN;

-- End previous active session if any
UPDATE roleplay_sessions
SET status = 'ended', updated_at = NOW()
WHERE user_id = '$user_id' AND status = 'active';

-- Create new session
INSERT INTO roleplay_sessions
    (id, user_id, character_id, scene_id, status, mood,
     relationship_level, message_count, created_at, updated_at)
VALUES
    (gen_random_uuid(), '$user_id', '$char_id', '$scene_id',
     'active', '$mood', '$rel', 0, NOW(), NOW())
RETURNING id;

COMMIT;
SQL
    )

    local new_session_id
    new_session_id=$(echo "$result" | grep -v '^$' | tail -1)

    echo -e "${GREEN}Session created:${RESET} $new_session_id"
    echo -e "  Scene: ${SELECTED_SCENE_NAME}"
    echo -e "  Mood: ${mood}, Relationship: ${rel}"
    echo ""
    print_user_status "$user_id" "$line_id" "$dname" "$terms_at"
}

action_restart_session() {
    select_user || return
    local user_id="$SELECTED_USER_ID"
    local line_id="$SELECTED_LINE_ID"
    local dname="$SELECTED_DISPLAY_NAME"
    local terms_at="$SELECTED_TERMS_AT"

    if ! get_active_session "$user_id"; then
        echo -e "${RED}No active session to restart.${RESET}"
        return
    fi

    local char_id="$SESSION_CHAR_ID"
    local scene_id="$SESSION_SCENE_ID"

    # Get scene defaults
    local scene_row
    scene_row=$(db_query "SELECT start_mood, start_relationship_level FROM scenes WHERE id = '$scene_id'")
    local mood rel
    IFS=$'\t' read -r mood rel <<< "$scene_row"

    # Transaction: end current + create new
    local result
    result=$(psql "$DATABASE_URL" -t -A -F $'\t' <<SQL
BEGIN;

UPDATE roleplay_sessions
SET status = 'ended', updated_at = NOW()
WHERE id = '$SESSION_ID';

INSERT INTO roleplay_sessions
    (id, user_id, character_id, scene_id, status, mood,
     relationship_level, message_count, created_at, updated_at)
VALUES
    (gen_random_uuid(), '$user_id', '$char_id', '$scene_id',
     'active', '$mood', '$rel', 0, NOW(), NOW())
RETURNING id;

COMMIT;
SQL
    )

    local new_session_id
    new_session_id=$(echo "$result" | grep -v '^$' | tail -1)

    echo -e "${YELLOW}Ended session:${RESET} $SESSION_ID"
    echo -e "${GREEN}Session restarted:${RESET} $new_session_id"
    echo -e "  Mood: ${mood}, Relationship: ${rel}"
    echo ""
    print_user_status "$user_id" "$line_id" "$dname" "$terms_at"
}

action_end_session() {
    select_user || return
    local user_id="$SELECTED_USER_ID"

    local ended
    ended=$(db_query "
        UPDATE roleplay_sessions
        SET status = 'ended', updated_at = NOW()
        WHERE user_id = '$user_id' AND status = 'active'
        RETURNING id
    ")

    if [[ -n "$ended" ]]; then
        echo -e "${GREEN}Session ended:${RESET} $ended"
    else
        echo -e "${YELLOW}No active session to end.${RESET}"
    fi

    print_user_status "$SELECTED_USER_ID" "$SELECTED_LINE_ID" "$SELECTED_DISPLAY_NAME" "$SELECTED_TERMS_AT"
}

action_set_credits() {
    select_user || return
    local user_id="$SELECTED_USER_ID"

    local old_balance
    old_balance=$(db_query "SELECT balance FROM credit_balances WHERE user_id = '$user_id'")

    if [[ -z "$old_balance" ]]; then
        echo -e "${RED}No credit balance found for user.${RESET}"
        return
    fi

    echo -e "Current balance: ${BOLD}${old_balance}${RESET}"
    echo -n "New balance amount: "
    local amount
    read -r amount

    if [[ -z "$amount" ]]; then
        return
    fi

    # Validate integer
    if ! [[ "$amount" =~ ^[0-9]+$ ]]; then
        echo -e "${RED}Invalid number.${RESET}"
        return
    fi

    db_query "UPDATE credit_balances SET balance = $amount, updated_at = NOW() WHERE user_id = '$user_id'"

    echo -e "${GREEN}Credits updated:${RESET} ${old_balance} → ${amount}"

    get_credit_balance "$user_id" || true
    print_credit_info
}

# ---------------------------------------------------------------------------
# Main menu
# ---------------------------------------------------------------------------
main() {
    echo -e "${BOLD}${CYAN}Simulation Webhook Test CLI${RESET}"
    echo -e "${DIM}Enter number to select${RESET}"
    echo ""

    load_env
    check_deps

    local options=(
        "List Users"
        "List Scenes"
        "Show User Status"
        "Accept Terms"
        "Create Session"
        "Restart Session"
        "End Session"
        "Set Credits"
        "Exit"
    )

    local PS3="What would you like to do? "
    while true; do
        echo ""
        select choice in "${options[@]}"; do
            case "$choice" in
                "List Users")       action_list_users ;;
                "List Scenes")      action_list_scenes ;;
                "Show User Status") action_show_status ;;
                "Accept Terms")     action_accept_terms ;;
                "Create Session")   action_create_session ;;
                "Restart Session")  action_restart_session ;;
                "End Session")      action_end_session ;;
                "Set Credits")      action_set_credits ;;
                "Exit")             echo -e "\n${DIM}Bye!${RESET}"; return ;;
                *)                  echo -e "${RED}Invalid option${RESET}"; continue ;;
            esac
            break
        done
    done
}

main
