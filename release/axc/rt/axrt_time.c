/* axrt_time.c -- axlang 运行时模块 */
#include "axrt.h"
#include "axrt_internal.h"


ax_str ax_time_to_str(int64_t ms) {
    return ax_time_format(ms, "%Y-%m-%d %H:%M:%S");
}


ax_str ax_time_format(int64_t ms, const char* fmt) {
    time_t sec = (time_t)(ms / 1000);
    struct tm tm;
#ifdef _WIN32
    localtime_s(&tm, &sec);
#else
    localtime_r(&sec, &tm);
#endif
    const char* f = fmt && *fmt ? fmt : "%Y-%m-%d %H:%M:%S";
    char buf[64];
    size_t n = strftime(buf, sizeof buf, f, &tm);
    return ax_str_from_bytes((const uint8_t*)buf, (int64_t)n);
}


void ax_time_sleep(int64_t ms) {
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    if (ms > 0) usleep((useconds_t)(ms * 1000));
#endif
}

/* time.Unix：sec 秒 + nsec 纳秒 -> Unix 毫秒 */
int64_t ax_time_from_unix(int64_t sec, int64_t nsec) {
    return sec * 1000 + nsec / 1000000;
}

/* time.UnixMilli：毫秒 -> Unix 毫秒（与 FromUnixMs 同义） */
int64_t ax_time_from_unix_ms(int64_t ms) {
    return ms;
}

/* time.Now：当前 Unix 毫秒（time 类型，与 ax_now_ms 同值） */
int64_t ax_time_now(void) {
    return ax_now_ms();
}

/* time.UnixMicro：微秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_us(int64_t us) {
    return us / 1000;
}

/* time.UnixNano：纳秒 -> Unix 毫秒 */
int64_t ax_time_from_unix_ns(int64_t nsec) {
    return nsec / 1000000;
}

/* time.Since：当前 Unix 毫秒 - t */
int64_t ax_time_since(int64_t t) {
    return ax_now_ms() - t;
}

/* time.Date：按 UTC 组装年月日时分秒纳秒 -> Unix 毫秒（标准 C11，纯手工计算，无时区/闰秒） */

/* time.Date：按 UTC 组装年月日时分秒纳秒 -> Unix 毫秒（标准 C11，纯手工计算，无时区/闰秒） */
static int ax_time_is_leap(int64_t y) {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) ? 1 : 0;
}

int64_t ax_time_date(int64_t year, int64_t month, int64_t day,
                     int64_t hour, int64_t min, int64_t sec, int64_t nsec) {
    static const int64_t mdays[12] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    int64_t days = 0;
    int64_t y;
    for (y = 1970; y < year; y++) days += ax_time_is_leap(y) ? 366 : 365;
    for (int64_t m = 1; m < month; m++) {
        days += mdays[m - 1];
        if (m == 2 && ax_time_is_leap(year)) days += 1;
    }
    days += (day - 1);
    return days * 86400000 +
           (hour * 3600 + min * 60 + sec) * 1000 +
           nsec / 1000000;
}

/* time.Until：t - 当前 Unix 毫秒（t 在将来为正，已过去为负） */
int64_t ax_time_until(int64_t t) {
    return t - ax_now_ms();
}


ax_res_time ax_time_parse_result(ax_str s, ax_str fmt) {
    ax_res_time r;
    int year = 1970, month = 1, day = 1, hour = 0, min = 0, sec = 0;
    const char* sp = (const char*)s.p;
    const char* fp = (const char*)fmt.p;
    int64_t slen = s.len, flen = fmt.len;
    int64_t si = 0, fi = 0;
    int ok = 1;

    while (fi < flen && si < slen && ok) {
        if (fp[fi] == '%' && fi + 1 < flen) {
            char spec = fp[fi + 1];
            fi += 2;
            int val = 0;
            int digits = 0;
            int max_digits = (spec == 'Y') ? 4 : 2;
            while (si < slen && sp[si] >= (uint8_t)'0' && sp[si] <= (uint8_t)'9' && digits < max_digits) {
                val = val * 10 + (sp[si] - (uint8_t)'0');
                si++;
                digits++;
            }
            if (digits == 0) { ok = 0; break; }
            switch (spec) {
                case 'Y': year = val; break;
                case 'm': month = val; break;
                case 'd': day = val; break;
                case 'H': hour = val; break;
                case 'M': min = val; break;
                case 'S': sec = val; break;
                default: ok = 0; break;
            }
            /* 跳过格式串中紧跟的非占位符分隔符 */
            while (fi < flen && fp[fi] != (uint8_t)'%' && fp[fi] >= (uint8_t)'0' && fp[fi] <= (uint8_t)'9') fi++;
        } else {
            if (sp[si] == fp[fi]) { si++; fi++; }
            else { fi++; } /* 宽松匹配：格式字符跳过，不强制输入一一对应 */
        }
    }

    if (!ok || year < 1970 || month < 1 || month > 12 || day < 1 || day > 31 ||
        hour < 0 || hour > 23 || min < 0 || min > 59 || sec < 0 || sec > 59) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("bad time format"));
        return r;
    }

    struct tm tm;
    memset(&tm, 0, sizeof(tm));
    tm.tm_year = year - 1900;
    tm.tm_mon = month - 1;
    tm.tm_mday = day;
    tm.tm_hour = hour;
    tm.tm_min = min;
    tm.tm_sec = sec;
    time_t t = mktime(&tm);
    if (t == (time_t)-1) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("bad time format"));
        return r;
    }
    r.tag = 1;
    r.u.ok = (int64_t)t * 1000;
    return r;
}

/* time.Round：按 d 毫秒间隔四舍五入到最近的整数倍，d <= 0 时返回 t 本身。
 * C11 整数除法向零截断，分正负处理避免负数舍入偏差。 */
int64_t ax_time_round(int64_t t, int64_t d) {
    if (d <= 0) return t;
    int64_t half = d / 2;
    if (t >= 0) {
        return ((t + half) / d) * d;
    } else {
        return -(((-t) + half) / d) * d;
    }
}

/* time.Truncate：按 d 毫秒间隔向零截断到整数倍，d <= 0 时返回 t 本身。
 * C11 整数除法本身即向零截断，故 (t / d) * d 即可。 */
int64_t ax_time_truncate(int64_t t, int64_t d) {
    if (d <= 0) return t;
    return (t / d) * d;
}

/* time.Now：当前 Unix 毫秒（time 类型） */
int64_t ax_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    /* 1601-01-01 → 1970-01-01：11644473600 秒 */
    int64_t t = ((int64_t)ft.dwHighDateTime << 32) | (int64_t)ft.dwLowDateTime;
    return t / 10000 - 11644473600000LL;
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000 + tv.tv_usec / 1000;
#endif
}