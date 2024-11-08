from numpy import genfromtxt
from matplotlib import pyplot as plt

def run():
    my_data = genfromtxt('lac_leman.csv', delimiter='\t', skip_header=1)
    print(my_data[:, 5])
    
    fig, ax = plt.subplots()
    ax.quiver(my_data[:, 5], -my_data[:, 6], my_data[:, 3], my_data[:, 4], angles='xy', scale_units='xy')
    # ax.plot(my_data[:, 5], -my_data[:, 6])

    #set aspect ratio to 1
    ratio = 1.0
    x_left, x_right = ax.get_xlim()
    y_low, y_high = ax.get_ylim()
    ax.set_aspect(abs((x_right-x_left)/(y_low-y_high))*ratio)

    # for i, _, _, _, _, x, y in enumerate(my_data):
    #     ax.annotate(i, (x, y))
    plt.show()