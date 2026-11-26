// ======================================================================
// 設定画面 監視・パイプライン UI 同期
// ======================================================================

import { loadedState } from './ctx.ts';
import { escapeHtml, modeLabel } from './html_builders.ts';
import { scheduleApply } from './ipc_apply.ts';
import { maxPipelineLength } from './limits.ts';

import SettingsSharedLogic from './shared_logic.ts';

/** 画像出力モード ID の集合を返す */
function imageModeIds(): Set<string> {
    return new Set(
        (loadedState().modes || [])
            .filter((entry) => entry.producesImage === true)
            .map((entry) => entry.id)
    );
}

/**
 * 加工パイプラインを正規化する
 *
 * 画像出力モードは末尾に1つのみ残し、最大段数を超える分は切り捨てる
 */
function normalizePipeline(pipeline: string[]): string[] {
    return SettingsSharedLogic.normalizePipeline(pipeline, maxPipelineLength(), imageModeIds());
}

/**
 * パイプライン変更を正規化・適用し UI を更新する
 */
export function applyPipelineChange(pipeline: string[]): void {
    const normalized = normalizePipeline(pipeline);
    loadedState().pipeline = normalized;
    scheduleApply({ pipeline: normalized });
    updateMonitorPipelineUi();
}

/** 監視タブの連続記録バナー表示を同期する */
function syncMonitorContinuousRecordBanner(): void {
    const banner = document.getElementById('monitor-record-banner');
    if (banner) {
        banner.hidden = !loadedState().continuousRecordEnabled;
    }
}

/** 連続記録中は加工パイプラインセクションを無効表示にする */
function syncMonitorProcessingSection(): void {
    const section = document.getElementById('monitor-processing-section');
    if (section) {
        section.classList.toggle('is-disabled', loadedState().continuousRecordEnabled);
    }
}

/** 履歴タブの連続記録バナー表示を同期する */
function syncRecordActiveBanner(): void {
    const banner = document.getElementById('record-active-banner');
    if (banner) {
        banner.hidden = !loadedState().continuousRecordEnabled;
    }
}

/** 連続記録関連 UI を一括同期する */
export function syncContinuousRecordUi(): void {
    syncMonitorContinuousRecordBanner();
    syncMonitorProcessingSection();
    syncRecordActiveBanner();
}

/** イベント監視時はポーリング間隔セクションを無効表示にする */
export function syncMonitorIntervalSection(): void {
    const section = document.getElementById('interval-section');
    if (section) {
        section.classList.toggle('disabled', loadedState().monitorMode === 'Event');
    }
}

/**
 * 加工パイプライン一覧の内部 HTML を組み立てる
 */
export function buildPipelineInnerHtml(): string {
    const pipeline = loadedState().pipeline || [];
    if (pipeline.length === 0) {
        return '<div class="empty-hint">パイプラインは未設定です (デフォルトモードのみ適用)</div>';
    }
    return (
        '<div class="pipeline-list">' +
        pipeline
            .map((modeId, index) => {
                return (
                    '<div class="pipeline-item" data-index="' +
                    index +
                    '">' +
                    '<span class="pipeline-step">' +
                    (index + 1) +
                    '</span>' +
                    '<span class="label">' +
                    escapeHtml(modeLabel(modeId)) +
                    '</span>' +
                    '<div class="pipeline-actions">' +
                    '<button type="button" class="icon-btn pipeline-up" data-index="' +
                    index +
                    '" title="上へ">↑</button>' +
                    '<button type="button" class="icon-btn pipeline-down" data-index="' +
                    index +
                    '" title="下へ">↓</button>' +
                    '<button type="button" class="icon-btn pipeline-remove" data-index="' +
                    index +
                    '" title="削除">×</button>' +
                    '</div></div>'
                );
            })
            .join('') +
        '</div>'
    );
}

/** 加工パイプライン一覧 DOM を再描画する */
function updateMonitorPipelineUi(): void {
    const container = document.getElementById('pipeline-list-container');
    if (!container) {
        return;
    }
    container.innerHTML = buildPipelineInnerHtml();
}
