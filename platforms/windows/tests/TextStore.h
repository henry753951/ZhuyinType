#pragma once
#include "Composition.h"
#include <textstor.h>
#include <inputscope.h>
#include <algorithm>
#include <string>
#include <vector>

// A real TSF-enabled native control: TSF owns its context and loads the registered
// COM service. Tests never call the service's key/composition interfaces directly.
class TextStore final : public ITextStoreACP, public ITfContextOwnerCompositionSink, public ITfInputScope {
public:
    HWND window=nullptr;
    std::wstring text;
    LONG start=0,end=0;
    bool password=false,composing=false,rejectComposition=false,denyLock=false;
    unsigned starts=0,ends=0,textExtCalls=0;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        if (!out) return E_POINTER;
        *out=nullptr;
        if (iid==IID_IUnknown || iid==IID_ITextStoreACP) *out=static_cast<ITextStoreACP*>(this);
        else if (iid==IID_ITfContextOwnerCompositionSink) *out=static_cast<ITfContextOwnerCompositionSink*>(this);
        else if (iid==IID_ITfInputScope) *out=static_cast<ITfInputScope*>(this);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs_; }
    STDMETHODIMP_(ULONG) Release() override { const auto n=--refs_; if (!n) delete this; return n; }
    STDMETHODIMP AdviseSink(REFIID iid,IUnknown* unknown,DWORD) override {
        if (iid!=IID_ITextStoreACPSink || !unknown) return E_INVALIDARG;
        return unknown->QueryInterface(IID_PPV_ARGS(&sink_));
    }
    STDMETHODIMP UnadviseSink(IUnknown*) override { sink_.Reset(); return S_OK; }
    STDMETHODIMP RequestLock(DWORD flags,HRESULT* result) override {
        if (!result || !sink_) return E_INVALIDARG;
        if (denyLock) { *result=TS_E_SYNCHRONOUS; return S_OK; }
        if (lock_) {
            if (flags&TS_LF_SYNC) *result=TS_E_SYNCHRONOUS;
            else { pendingLock_|=flags; *result=TS_S_ASYNC; }
            return S_OK;
        }
        lock_=flags;
        *result=sink_->OnLockGranted(flags);
        lock_=0;
        while (pendingLock_) {
            lock_=pendingLock_; pendingLock_=0;
            sink_->OnLockGranted(lock_); lock_=0;
        }
        return S_OK;
    }
    STDMETHODIMP GetStatus(TS_STATUS* status) override { if (!status) return E_POINTER; *status={0,0}; return S_OK; }
    STDMETHODIMP QueryInsert(LONG a,LONG b,ULONG,LONG* x,LONG* y) override {
        if (!Valid(a,b) || !x || !y) return E_INVALIDARG; *x=a; *y=b; return S_OK;
    }
    STDMETHODIMP GetSelection(ULONG index,ULONG count,TS_SELECTION_ACP* selection,ULONG* fetched) override {
        if (!lock_) return TS_E_NOLOCK;
        if (!selection || !fetched || !count || (index!=0 && index!=TS_DEFAULT_SELECTION)) return E_INVALIDARG;
        *selection={start,end,{TS_AE_END,FALSE}}; *fetched=1; return S_OK;
    }
    STDMETHODIMP SetSelection(ULONG count,const TS_SELECTION_ACP* selection) override {
        if (!Writable()) return TS_E_NOLOCK;
        if (count!=1 || !selection || !Valid(selection->acpStart,selection->acpEnd)) return E_INVALIDARG;
        start=selection->acpStart; end=selection->acpEnd; InvalidateRect(window,nullptr,TRUE); return S_OK;
    }
    STDMETHODIMP GetText(LONG a,LONG b,WCHAR* buffer,ULONG capacity,ULONG* copied,TS_RUNINFO* runs,ULONG runCapacity,ULONG* runCount,LONG* next) override {
        if (!lock_) return TS_E_NOLOCK;
        if (b==-1) b=static_cast<LONG>(text.size());
        if (!Valid(a,b) || !copied || !runCount || !next) return E_INVALIDARG;
        const auto count=std::min(static_cast<ULONG>(b-a),capacity);
        if (count && !buffer) return E_INVALIDARG;
        if (count) std::copy_n(text.data()+a,count,buffer);
        *copied=count; *next=a+static_cast<LONG>(count); *runCount=0;
        if (runCapacity && runs) { *runs={count,TS_RT_PLAIN}; *runCount=1; }
        return S_OK;
    }
    STDMETHODIMP SetText(DWORD,LONG a,LONG b,const WCHAR* value,ULONG count,TS_TEXTCHANGE* change) override {
        if (!Writable()) return TS_E_NOLOCK;
        if (!Valid(a,b) || !change || (count && !value)) return E_INVALIDARG;
        try { text.replace(a,b-a,value?value:L"",count); } catch (...) { return E_OUTOFMEMORY; }
        *change={a,b,a+static_cast<LONG>(count)};
        start=end=change->acpNewEnd; InvalidateRect(window,nullptr,TRUE); return S_OK;
    }
    STDMETHODIMP GetFormattedText(LONG,LONG,IDataObject** out) override { if(out) *out=nullptr; return E_NOTIMPL; }
    STDMETHODIMP GetEmbedded(LONG,REFGUID,REFIID,IUnknown** out) override { if(out) *out=nullptr; return E_NOTIMPL; }
    STDMETHODIMP QueryInsertEmbedded(const GUID*,const FORMATETC*,BOOL* insert) override { if (!insert) return E_POINTER; *insert=FALSE; return S_OK; }
    STDMETHODIMP InsertEmbedded(DWORD,LONG,LONG,IDataObject*,TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    STDMETHODIMP InsertTextAtSelection(DWORD flags,const WCHAR* value,ULONG count,LONG* a,LONG* b,TS_TEXTCHANGE* change) override {
        if (!Writable()) return TS_E_NOLOCK;
        if (a) *a=start; if (b) *b=end;
        if (flags&TS_IAS_QUERYONLY) return S_OK;
        return SetText(0,start,end,value,count,change);
    }
    STDMETHODIMP InsertEmbeddedAtSelection(DWORD,IDataObject*,LONG*,LONG*,TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    STDMETHODIMP RequestSupportedAttrs(DWORD,ULONG count,const TS_ATTRID* ids) override {
        scopeRequested_=false;
        for (ULONG i=0;i<count;++i) if (ids[i]==GUID_PROP_INPUTSCOPE) scopeRequested_=true;
        return S_OK;
    }
    STDMETHODIMP RequestAttrsAtPosition(LONG,ULONG count,const TS_ATTRID* ids,DWORD flags) override { return RequestSupportedAttrs(flags,count,ids); }
    STDMETHODIMP RequestAttrsTransitioningAtPosition(LONG,ULONG,const TS_ATTRID*,DWORD) override { scopeRequested_=false; return S_OK; }
    STDMETHODIMP FindNextAttrTransition(LONG,LONG halt,ULONG,const TS_ATTRID*,DWORD,LONG* next,BOOL* found,LONG* offset) override {
        if (!next || !found || !offset) return E_POINTER; *next=halt; *found=FALSE; *offset=0; return S_OK;
    }
    STDMETHODIMP RetrieveRequestedAttrs(ULONG count,TS_ATTRVAL* attrs,ULONG* fetched) override {
        if (!fetched) return E_POINTER; *fetched=0;
        if (scopeRequested_ && count && attrs) {
            attrs[0].idAttr=GUID_PROP_INPUTSCOPE; attrs[0].dwOverlapId=0;
            VariantInit(&attrs[0].varValue); attrs[0].varValue.vt=VT_UNKNOWN;
            attrs[0].varValue.punkVal=static_cast<ITfInputScope*>(this); AddRef(); *fetched=1;
        }
        scopeRequested_=false; return S_OK;
    }
    STDMETHODIMP GetEndACP(LONG* result) override { if (!result) return E_POINTER; if (!lock_) return TS_E_NOLOCK; *result=static_cast<LONG>(text.size()); return S_OK; }
    STDMETHODIMP GetActiveView(TsViewCookie* result) override { if (!result) return E_POINTER; *result=1; return S_OK; }
    STDMETHODIMP GetACPFromPoint(TsViewCookie,const POINT*,DWORD,LONG* result) override { if (!result) return E_POINTER; *result=end; return S_OK; }
    STDMETHODIMP GetTextExt(TsViewCookie,LONG a,LONG b,RECT* rect,BOOL* clipped) override {
        if (!lock_) return TS_E_NOLOCK;
        if (!rect || !clipped || !Valid(a,b)) return E_INVALIDARG;
        *rect={20+12*a,60,22+12*b,88}; MapWindowPoints(window,nullptr,reinterpret_cast<POINT*>(rect),2);
        *clipped=FALSE; ++textExtCalls; return S_OK;
    }
    STDMETHODIMP GetScreenExt(TsViewCookie,RECT* rect) override { if (!rect) return E_POINTER; GetWindowRect(window,rect); return S_OK; }
    STDMETHODIMP GetWnd(TsViewCookie,HWND* result) override { if (!result) return E_POINTER; *result=window; return S_OK; }
    STDMETHODIMP OnStartComposition(ITfCompositionView*,BOOL* accept) override {
        if (!accept) return E_POINTER; *accept=!rejectComposition; if (*accept) { composing=true; ++starts; } return S_OK;
    }
    STDMETHODIMP OnUpdateComposition(ITfCompositionView*,ITfRange*) override { return S_OK; }
    STDMETHODIMP OnEndComposition(ITfCompositionView*) override { composing=false; ++ends; return S_OK; }
    STDMETHODIMP GetInputScopes(InputScope** scopes,UINT* count) override {
        if (!scopes || !count) return E_POINTER;
        *scopes=static_cast<InputScope*>(CoTaskMemAlloc(sizeof(InputScope)));
        if (!*scopes) return E_OUTOFMEMORY;
        **scopes=password?IS_PASSWORD:IS_DEFAULT; *count=1; return S_OK;
    }
    STDMETHODIMP GetPhrase(BSTR** phrases,UINT* count) override { if(phrases) *phrases=nullptr; if(count) *count=0; return E_NOTIMPL; }
    STDMETHODIMP GetRegularExpression(BSTR* out) override { if(out) *out=nullptr; return E_NOTIMPL; }
    STDMETHODIMP GetSRGS(BSTR* out) override { if(out) *out=nullptr; return E_NOTIMPL; }
    STDMETHODIMP GetXML(BSTR* out) override { if(out) *out=nullptr; return E_NOTIMPL; }
    void Replace(const std::wstring& value) {
        TS_TEXTCHANGE change{0,static_cast<LONG>(text.size()),static_cast<LONG>(value.size())};
        text=value; start=end=static_cast<LONG>(value.size());
        if (sink_) { sink_->OnTextChange(0,&change); sink_->OnSelectionChange(); }
        InvalidateRect(window,nullptr,TRUE);
    }
    void Password(bool value) {
        password=value;
        if (sink_) { const TS_ATTRID id=GUID_PROP_INPUTSCOPE; sink_->OnAttrsChange(0,static_cast<LONG>(text.size()),1,&id); }
    }
private:
    bool Valid(LONG a,LONG b) const { return a>=0 && b>=a && static_cast<size_t>(b)<=text.size(); }
    bool Writable() const { return (lock_&TS_LF_READWRITE)==TS_LF_READWRITE; }
    ULONG refs_=1;
    DWORD lock_=0,pendingLock_=0;
    bool scopeRequested_=false;
    zt::Com<ITextStoreACPSink> sink_;
};
