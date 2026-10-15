import marimo

__generated_with = "0.23.16"
app = marimo.App()


@app.cell
def _():
    import json
    import numpy as np
    import pandas as pd
    import plotly.express as px
    import plotly.graph_objects as go

    def read_trace32_printftrace(f):
        text = "".join([l[13:].rstrip() for l in f.readlines()[2:]])

        decoder = json.JSONDecoder()
        data = []
        while text:
            obj, idx = decoder.raw_decode(text)
            data.append(obj)
            text = text[idx:].lstrip()

        return data

    return go, np, pd, px, read_trace32_printftrace


@app.cell
def _(pd, read_trace32_printftrace):
    with open("foo-2.txt") as f:
        data = read_trace32_printftrace(f)

    data = pd.json_normalize(data)

    data["timestamp"] = data["timestamp"] - data["timestamp"][0]
    data["timestamp"] = pd.to_datetime(data["timestamp"], unit="us")

    data["intruder_set_mask"] = data["intruder_set_mask"].astype(str)

    data["ipf"] = data["perf_info.instr"] / data["df"]

    data["cpi"] = data["perf_info.cycles"] / data["perf_info.instr"]
    data["api"] = data["perf_info.l1d_access"] / data["perf_info.instr"]

    data["l1_miss_ratio"] = data["perf_info.l2d_access"] / data["perf_info.l1d_access"]
    data["l2_miss_ratio"] = data["perf_info.l2d_refill"] / data["perf_info.l2d_access"]

    dt_baseline = data[data["intruder_set_mask"] == "0"]["dt"].median()
    data["rel_impact"] = data["dt"] / dt_baseline

    data
    return (data,)


@app.cell
def _(data, px):
    px.scatter(data, x="l2_miss_ratio", y="rel_impact", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    med = data.groupby("intruder_set_mask").agg({"l2_miss_ratio": "median", "rel_impact": "median"}).reset_index()
    px.scatter(med, x="l2_miss_ratio", y="rel_impact", color="intruder_set_mask")
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
 
    """)
    return


@app.cell
def _(data, px):

    px.scatter(data, x="intruder_set_mask", y="rel_impact", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    px.scatter(data, x="l2_miss_ratio", y="l1_miss_ratio", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    px.box(data, x="api", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    px.histogram(data, x="dt", histnorm='probability density')
    return


@app.cell
def _(data):
    from statsmodels.tsa.stattools import kpss

    x = data[data["intruder_set_mask"] == "0"]["dt"]
    x
    return kpss, x


@app.cell
def _(kpss, x):

    kpss(x, regression="c", result_object=True)

    return


@app.cell
def _(data):
    from pyextremes import EVA

    y= data.set_index("timestamp")

    return EVA, y


@app.cell
def _(EVA, y):


    model = EVA(y["dt"])
    model.get_extremes(method="BM", block_size="5000us", errors="coerce")
    model.fit_model()
    model.get_summary(return_period_size="1us", return_period=[10000, 100000, 1000000, 100000000000000])

    return


@app.cell
def _(data, go, np):
    from scipy.stats import genpareto

    gpd = genpareto.fit(data[data["dt"] > 2600]["dt"])
    print(gpd)

    xline = np.arange(2000, 3000, 1)
    yline = genpareto.pdf(xline, *gpd)
    zline = genpareto.cdf(xline, *gpd)

    fig = go.Figure()
    fig.add_trace(go.Histogram(x=data["dt"], histnorm="probability density"))
    fig.add_trace(go.Line(x=xline,y=yline))


    fig
    return xline, zline


@app.cell
def _(go, xline, zline):
    fig2 = go.Figure()
    fig2.add_trace(go.Line(x=xline,y=zline))
    fig2
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
