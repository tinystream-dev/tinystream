// SPDX-License-Identifier: AGPL-3.0-or-later

import { useState } from 'react'
import { Page } from './Page'

/** Whether content has just replaced a skeleton (it wasn't ready when this first rendered). */
export function useArrived(ready: boolean) {
  const [cold] = useState(!ready)
  return cold && ready
}
import { PosterGrid } from './PosterGrid'

export function Bone({ className = '' }: { className?: string }) {
  return <div className={`bone rounded-xl ${className}`} />
}

export function PosterGridSkeleton({ count = 18 }: { count?: number }) {
  return (
    <PosterGrid>
      {Array.from({ length: count }, (_, i) => (
        <div key={i}>
          <Bone className="aspect-[2/3] rounded-[14px]" />
          <Bone className="mt-2 h-3 w-3/4 rounded-md" />
          <Bone className="mt-1.5 h-2.5 w-1/3 rounded-md" />
        </div>
      ))}
    </PosterGrid>
  )
}

export function RowSkeleton({ wide = false }: { wide?: boolean }) {
  return (
    <div className="mb-11">
      <Bone className="mb-4 h-4 w-40 rounded-md" />
      <div className="flex gap-5 overflow-hidden">
        {Array.from({ length: 8 }, (_, i) =>
          wide ? (
            <Bone key={i} className="aspect-video w-80 shrink-0 rounded-2xl" />
          ) : (
            <Bone key={i} className="aspect-[2/3] w-38 shrink-0 rounded-[14px]" />
          ),
        )}
      </div>
    </div>
  )
}

export function HomeSkeleton() {
  return (
    <Page>
      <Bone className="mb-11 h-50 rounded-[22px] md:h-59" />
      <RowSkeleton wide />
      <RowSkeleton />
    </Page>
  )
}

export function ListSkeleton({ rows = 4, title = true }: { rows?: number; title?: boolean }) {
  return (
    <Page>
      {title && <Bone className="mb-8 h-8 w-48 rounded-lg" />}
      <div className="space-y-2">
        {Array.from({ length: rows }, (_, i) => (
          <Bone key={i} className="h-24 rounded-2xl" />
        ))}
      </div>
    </Page>
  )
}

export function TitleSkeleton() {
  return (
    <Page>
      <div className="flex flex-col gap-8 pt-6 sm:flex-row sm:items-end md:pt-20">
        <Bone className="aspect-[2/3] w-40 shrink-0 rounded-[18px] sm:w-52" />
        <div className="flex-1 pb-1">
          <Bone className="h-11 w-2/3 rounded-lg" />
          <Bone className="mt-4 h-4 w-1/3 rounded-md" />
          <div className="mt-7 flex gap-2">
            <Bone className="h-11 w-36 rounded-[14px]" />
            <Bone className="h-11 w-36 rounded-[14px]" />
          </div>
        </div>
      </div>
      <Bone className="mt-8 h-20 max-w-[68ch] rounded-lg" />
      <div className="mt-12 space-y-2">
        {Array.from({ length: 4 }, (_, i) => (
          <div key={i} className="flex gap-4 p-3">
            <Bone className="aspect-video w-36 shrink-0 rounded-[10px] md:w-44" />
            <div className="flex-1 pt-1">
              <Bone className="h-4 w-1/2 rounded-md" />
              <Bone className="mt-2.5 h-3 w-5/6 rounded-md" />
            </div>
          </div>
        ))}
      </div>
    </Page>
  )
}
